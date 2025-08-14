
use crate::{cs_command, cs_command_data_sequence, cs_data, st7789v2::common::{ColorMode, Commands}};
use cortex_m::delay::Delay;
use defmt::debug;
use stm32f4xx_hal::{
    dma::{
        ChannelX, MemoryToPeripheral, StreamX, Transfer,
        config::DmaConfig,
        traits::{Channel, DMASet, Stream, StreamISR},
    },
    hal::digital::OutputPin,
    rcc,
    spi::{Instance, Tx},
};

// Note: CASET and RASET buffers are now user-provided via singleton!
// This makes memory allocation explicit and user-controlled

// Macro for handling CS timing with commands

pub const CHUNK_SIZE: usize = 1024 * 4;

pub struct ST7789V2DMA<
    'a,
    SPI,
    DMA: rcc::Enable + rcc::Reset,
    CS: OutputPin,
    DC: OutputPin,
    RST: OutputPin,
    const CHANNEL: u8,
    const S: u8,
    const W: usize = 240,
    const H: usize = 280,
    const OFFSET: usize = 20,
> where
    SPI: Instance + DMASet<StreamX<DMA, S>, CHANNEL, MemoryToPeripheral>,
{
    pub(super) cs: CS,
    pub(super) dc: DC,
    rst: RST,
    pub(super) tx: Option<Tx<SPI>>,
    pub(super) st: Option<StreamX<DMA, S>>,
    pub d: &'a mut Delay,
    cmd_buf: Option<&'static mut [u8; 1]>,
    data_buf: Option<&'static mut [u8; 1]>,
    caset_buf: Option<&'static mut [u8; 4]>, // Column address set buffer (user-provided)
    raset_buf: Option<&'static mut [u8; 4]>, // Row address set buffer (user-provided)
    pub(super) chunk_buffer: Option<&'static mut [u8; CHUNK_SIZE]>,
}

impl<'a, SPI, DMA, CS, DC, RST, const CHANNEL: u8, const S: u8, const W: usize, const H: usize, const OFFSET: usize>
    ST7789V2DMA<'a, SPI, DMA, CS, DC, RST, CHANNEL, S, W, H, OFFSET>
where
    SPI: Instance + DMASet<StreamX<DMA, S>, CHANNEL, MemoryToPeripheral>,
    CS: OutputPin,
    DC: OutputPin,
    RST: OutputPin,
    DMA: rcc::Enable + rcc::Reset + stm32f4xx_hal::dma::traits::Instance,
    StreamX<DMA, S>: Stream,
    ChannelX<CHANNEL>: Channel,
{
    pub fn new(
        cs: CS,
        dc: DC,
        rst: RST,
        tx: Tx<SPI>,
        st: StreamX<DMA, S>,
        d: &'a mut Delay,
        cmd_buf: &'static mut [u8; 1],
        data_buf: &'static mut [u8; 1],
        caset_buf: &'static mut [u8; 4], // User-provided column address buffer
        raset_buf: &'static mut [u8; 4], // User-provided row address buffer
        chunk_buffer: &'static mut [u8; CHUNK_SIZE],
    ) -> Self {
        Self {
            cs,
            dc,
            rst,
            tx: Some(tx),
            st: Some(st),
            d,
            cmd_buf: Some(cmd_buf),
            data_buf: Some(data_buf),
            caset_buf: Some(caset_buf),
            raset_buf: Some(raset_buf),
            chunk_buffer: Some(chunk_buffer),
        }
    }

    pub fn init(&mut self){
        // Initialization sequence for ST7789V2
        // This method should be called after creating the instance to initialize the display.
        // Order of commands:
        // 1. Software reset
        // 2. Sleep out
        // 3. Set color mode
        // 4. Memory data access control
        // 5. Display on

        self.rst.set_low().ok();
        self.d.delay_ms(120);
        self.rst.set_high().ok();
        self.d.delay_ms(150);
        debug!("Hardware reset completed in init()");

        // Use macros for proper CS timing - CS stays low during delay for command processing
        cs_command!(self, Commands::SoftwareReset, 150);
        debug!("Software reset step completed in init()");

        cs_command!(self, Commands::SleepOut, 120);
        debug!("Sleep out step completed in init()");

        cs_command!(self, Commands::SetColorMode, 1);
        cs_data!(self, ColorMode::RGB565 as u8, 10);
        debug!("Set color mode step completed in init()");

        cs_command!(self, Commands::MemoryDataAccessControl, 1);
        cs_data!(self, 0b0000_0000, 10); // Set to normal mode (no rotation)
        debug!("Memory data access control step completed in init()");

        cs_command!(self, Commands::InversionOn, 1);
        debug!("Inversion on step completed in init()");

        cs_command!(self, Commands::DisplayOn, 50);
        debug!("Display on step completed in init()");

    }

    pub fn set_size(&mut self, xs: u16, xe: u16, ys: u16, ye: u16) {
        // sets CASET and RASET based on given width and height
        // accounts for offset based on OFFSET

        let actual_ys = ys + OFFSET as u16;
        let actual_ye = ye + OFFSET as u16;

        let caset_buf = self.caset_buf.take().unwrap();
        let raset_buf = self.raset_buf.take().unwrap();

        caset_buf[0] = (xs >> 8) as u8; // Start column MSB
        caset_buf[1] = (xs & 0xFF) as u8; // Start column LSB
        caset_buf[2] = (xe >> 8) as u8; // End column MSB
        caset_buf[3] = (xe & 0xFF) as u8; // End column LSB

        raset_buf[0] = (actual_ys >> 8) as u8; // Start row MSB
        raset_buf[1] = (actual_ys & 0xFF) as u8; // Start row LSB
        raset_buf[2] = (actual_ye >> 8) as u8; // End row MSB
        raset_buf[3] = (actual_ye & 0xFF) as u8; // End row LSB

        self.caset_buf = Some(caset_buf);
        self.raset_buf = Some(raset_buf);

        cs_command_data_sequence!(self, Commands::CASET, send_caset_data_safe, 1, 1);
        cs_command_data_sequence!(self, Commands::RASET, send_raset_data_safe, 1, 1);

    }

    #[inline(always)]
    pub fn begin_draw(&mut self){
        cs_command!(self, Commands::RAMWR, 10);
    }

    pub fn off(&mut self) {
        cs_command!(self, Commands::DisplayOff, 50);
    }

    fn send_command(&mut self, cmd: Commands) {
        let cmd_buf = self.cmd_buf.take().unwrap();
        cmd_buf[0] = cmd as u8;

        let st = self.st.take().unwrap();
        let tx = self.tx.take().unwrap();

        // Set DC mode (CS is handled externally by macro)
        self.dc.set_low().ok(); // Command mode

        let config = DmaConfig::default()
            .peripheral_increment(false)
            .memory_increment(true)
            .fifo_enable(false)
            .transfer_complete_interrupt(false);


        let mut tf =
            Transfer::init_memory_to_peripheral(st, tx, cmd_buf, None, config);
        tf.start(|_| {});
        tf.wait();

        // Check for transfer errors
        if tf.is_transfer_error() {
            debug!(
                "ERROR: Transfer error detected in send_command for cmd 0x{:02X}",
                cmd as u8
            );
        } else {
            debug!(
                "SUCCESS: Command 0x{:02X} transfer completed without errors",
                cmd as u8
            );
        }

        let (st, tx, cmd_buf, _) = tf.release();
        self.st = Some(st);
        self.tx = Some(tx);
        self.cmd_buf = Some(cmd_buf);

        // CS stays low for external delay handling
    }

    fn send_data_u8(&mut self, data: u8){
        let data_buf = self.data_buf.take().unwrap();
        data_buf[0] = data;

        let st = self.st.take().unwrap();
        let tx = self.tx.take().unwrap();

        // Set DC mode (CS is handled externally by macro)
        self.dc.set_high().ok(); // Data mode

        let config = DmaConfig::default()
            .peripheral_increment(false)
            .memory_increment(true)
            .fifo_enable(false)
            .transfer_complete_interrupt(false);

        let mut tf =
            Transfer::init_memory_to_peripheral(st, tx, data_buf, None, config);
        tf.start(|_| {});
        tf.wait();

        // Check for transfer errors
        if tf.is_transfer_error() {
            debug!(
                "ERROR: Transfer error detected in send_data_u8 for data 0x{:02X}",
                data
            );
        } else {
            debug!(
                "SUCCESS: Data 0x{:02X} transfer completed without errors",
                data
            );
        }

        let (st, tx, data_buf, _) = tf.release();
        self.st = Some(st);
        self.tx = Some(tx);
        self.data_buf = Some(data_buf);

        // CS stays low for external delay handling
    }

    fn send_caset_data_safe(&mut self, delay_ms: u32){
        // CS is already LOW from macro, just send data
        self.dc.set_high().ok(); // Data mode
        
        let config = DmaConfig::default()
            .peripheral_increment(false)
            .memory_increment(true)
            .fifo_enable(false)
            .transfer_complete_interrupt(false);

        let st = self.st.take().unwrap();
        let tx = self.tx.take().unwrap();
        let caset_buf = self.caset_buf.take().unwrap();

        let mut tf = Transfer::init_memory_to_peripheral(st, tx, caset_buf, None, config);
        tf.start(|_| {});
        tf.wait();
        let (st, tx, caset_buf, _) = tf.release();
        self.st = Some(st); // restoring the updated copy.
        self.tx = Some(tx);
        self.caset_buf = Some(caset_buf);

        self.d.delay_ms(delay_ms); // Data processing delay
    }

    fn send_raset_data_safe(&mut self, delay_ms: u32){
        // CS is already LOW from macro, just send data
        self.dc.set_high().ok(); // Data mode
        
        let config = DmaConfig::default()
            .peripheral_increment(false)
            .memory_increment(true)
            .fifo_enable(false)
            .transfer_complete_interrupt(false);

        let st = self.st.take().unwrap();
        let tx = self.tx.take().unwrap();
        let raset_buf = self.raset_buf.take().unwrap();

        let mut tf = Transfer::init_memory_to_peripheral(st, tx, raset_buf, None, config);
        tf.start(|_| {});
        tf.wait();
        let (st, tx, raset_buf, _) = tf.release();
        self.st = Some(st);
        self.tx = Some(tx);
        self.raset_buf = Some(raset_buf);

        self.d.delay_ms(delay_ms); // Data processing delay
    }

    pub fn send_data_chunk(&mut self, chunk: &'static mut [u8; CHUNK_SIZE]) -> &'static mut [u8; CHUNK_SIZE] {
        let config = DmaConfig::default()
            .peripheral_increment(false)
            .memory_increment(true)
            .fifo_enable(false)
            .transfer_complete_interrupt(false);

        let st = self.st.take().unwrap();
        let tx = self.tx.take().unwrap();

        let mut tf = Transfer::init_memory_to_peripheral(st, tx, chunk, None, config);
        tf.start(|_| {});
        tf.wait();
        let (st, tx, d, _) = tf.release();
        self.st = Some(st);
        self.tx = Some(tx);
        d
    }

    #[inline(always)]
    pub fn select(&mut self) -> &mut Self {
        self.cs.set_low().ok(); // Select the device
        self
    }

    #[inline(always)]
    pub fn deselect(&mut self) -> &mut Self {
        self.cs.set_high().ok(); // Deselect the device
        self
    }

    // Additional methods for DMA operations can be added here
}
