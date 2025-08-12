use core::mem::{self, transmute};

use crate::{cs_command, cs_command_data_sequence, cs_data, st7789v2::common::{ColorMode, Commands}};
use cortex_m::delay::Delay;
use defmt::debug;
use embedded_graphics::{pixelcolor::Rgb565, prelude::{Dimensions, DrawTarget, OriginDimensions, PointsIter, Size}, Drawable};
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
    cs: CS,
    dc: DC,
    rst: RST,
    tx: Tx<SPI>,
    st: StreamX<DMA, S>,
    pub d: &'a mut Delay,
    cmd_buf: &'static mut [u8; 1],
    data_buf: &'static mut [u8; 1],
    caset_buf: &'static mut [u8; 4], // Column address set buffer (user-provided)
    raset_buf: &'static mut [u8; 4], // Row address set buffer (user-provided)
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
    ) -> Self {
        Self {
            cs,
            dc,
            rst,
            tx,
            st,
            d,
            cmd_buf: cmd_buf,
            data_buf: data_buf,
            caset_buf,
            raset_buf,
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

    pub fn draw_entire_screen(&mut self, buffer: &'static [u8]){
        // Display has OFFSET non-visible rows at top and bottom
        // So visible area is from row OFFSET to row (OFFSET + H - 1)
        let x_start = 0u16; // Start at column 0
        let x_end = W as u16 - 1; // End at column (W-1)

        let y_start = OFFSET as u16; // Start at row OFFSET (skip first OFFSET non-visible rows)
        let y_end = y_start + H as u16 - 1; // End at row (OFFSET + H - 1)

        // Prepare CASET data (Column Address Set) using member buffer
        self.caset_buf[0] = (x_start >> 8) as u8; // Start column MSB
        self.caset_buf[1] = (x_start & 0xFF) as u8; // Start column LSB
        self.caset_buf[2] = (x_end >> 8) as u8; // End column MSB
        self.caset_buf[3] = (x_end & 0xFF) as u8; // End column LSB

        // Prepare RASET data (Row Address Set) using member buffer
        self.raset_buf[0] = (y_start >> 8) as u8; // Start row MSB
        self.raset_buf[1] = (y_start & 0xFF) as u8; // Start row LSB
        self.raset_buf[2] = (y_end >> 8) as u8; // End row MSB
        self.raset_buf[3] = (y_end & 0xFF) as u8; // End row LSB

        debug!(
            "Drawing {}x{} image at position ({},{}) to ({},{}) with offset {}",
            W, H, x_start, y_start, x_end, y_end, OFFSET
        );

        // Send commands and address setup using unified macro for proper CS timing
        cs_command_data_sequence!(self, Commands::CASET, send_caset_data_safe, 1, 1);
        debug!("Column address set command sent");

        cs_command_data_sequence!(self, Commands::RASET, send_raset_data_safe, 1, 1);
        debug!("Row address set command sent");

        cs_command!(self, Commands::RAMWR, 10);

        // Now send the image data in chunks with CS low for entire transfer (like working code)
        self.dc.set_high().ok(); // Set data mode for image data
        self.cs.set_low().ok(); // Select device for entire transfer

        let chunk_size = 32 * 1024; // 32KB chunks

        for chunk in buffer.chunks(chunk_size) {
            self.send_data_raw(chunk);
        }

        self.cs.set_high().ok(); // Deselect device after all chunks
    }

    pub fn set_size(&mut self, xs: u16, xe: u16, ys: u16, ye: u16) {
        // sets CASET and RASET based on given width and height
        // accounts for offset based on OFFSET

        let actual_ys = ys + OFFSET as u16;
        let actual_ye = ye + OFFSET as u16;

        self.caset_buf[0] = (xs >> 8) as u8; // Start column MSB
        self.caset_buf[1] = (xs & 0xFF) as u8; // Start column LSB
        self.caset_buf[2] = (xe >> 8) as u8; // End column MSB
        self.caset_buf[3] = (xe & 0xFF) as u8; // End column LSB

        self.raset_buf[0] = (actual_ys >> 8) as u8; // Start row MSB
        self.raset_buf[1] = (actual_ys & 0xFF) as u8; // Start row LSB
        self.raset_buf[2] = (actual_ye >> 8) as u8; // End row MSB
        self.raset_buf[3] = (actual_ye & 0xFF) as u8; // End row LSB

        cs_command_data_sequence!(self, Commands::CASET, send_caset_data_safe, 1, 1);
        cs_command_data_sequence!(self, Commands::RASET, send_raset_data_safe, 1, 1);

    }

    pub fn begin_draw(&mut self){
        cs_command!(self, Commands::RAMWR, 10);
    }

    pub fn send_frame(&mut self, buffer: &'static [u8]){
        // must ensure begin_draw is called, before this method externally
        // buffer length must be correct as per selected width and height
        let chunk_size = 32 * 1024; // 32KB chunks

        self.dc.set_high().ok(); // Set data mode for image data
        self.cs.set_low().ok(); // Select device for entire transfer

        for chunk in buffer.chunks(chunk_size) {
            self.send_data_raw(chunk);
        }

        self.cs.set_high().ok(); // Deselect device after all chunks

    }

    pub fn draw_color_entire_screen(&mut self, color: u16){
        // draws color without using large buffer by tiling to save memory
        // creates small 2Kb buffer to hold color value and sends it repeatedly
        let color_buffer = [color as u8; 2 * 1024]; // 2KB buffer
        self.set_size(0, 240, 0, 320);
        self.begin_draw();

        let data: &'static [u8] = unsafe { transmute(color_buffer.as_slice()) }; // because the transfer doesnt outlive this

        self.dc.set_high().ok(); // Set data mode for image data
        self.cs.set_low().ok(); // Select device for entire transfer

        for _ in 0..((W * H * 2) / (2 * 1024) + 1) { // +1 to account for any remaining data
            self.send_data_raw(data)
        }

        self.cs.set_high().ok(); // Deselect device after all chunks

    }

    pub fn off(&mut self) {
        cs_command!(self, Commands::DisplayOff, 50);
    }

    fn send_command(&mut self, cmd: Commands) {
        self.cmd_buf[0] = cmd as u8;

        // Set DC mode (CS is handled externally by macro)
        self.dc.set_low().ok(); // Command mode

        let config = DmaConfig::default()
            .peripheral_increment(false)
            .memory_increment(true)
            .fifo_enable(false)
            .transfer_complete_interrupt(false);

        // Use unsafe transmute to avoid lifetime issues with DMA transfer
        let dup_st: StreamX<DMA, S> = unsafe{mem::transmute_copy(&self.st)};
        let dup_tx: Tx<SPI> = unsafe{mem::transmute_copy(&self.tx)};
        let dup_cmd_buf: &'static mut [u8; 1] = unsafe{mem::transmute_copy(&self.cmd_buf)};

        let mut tf =
            Transfer::init_memory_to_peripheral(dup_st, dup_tx, dup_cmd_buf, None, config);
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
        self.st = st;
        self.tx = tx;
        self.cmd_buf = cmd_buf;

        // CS stays low for external delay handling
    }

    fn send_data_u8(&mut self, data: u8){
        self.data_buf[0] = data;

        // Set DC mode (CS is handled externally by macro)
        self.dc.set_high().ok(); // Data mode

        let config = DmaConfig::default()
            .peripheral_increment(false)
            .memory_increment(true)
            .fifo_enable(false)
            .transfer_complete_interrupt(false);

        let dup_st: StreamX<DMA, S> = unsafe{mem::transmute_copy(&self.st)};
        let dup_tx: Tx<SPI> = unsafe{mem::transmute_copy(&self.tx)};
        let dup_data_buf: &'static mut [u8; 1] = unsafe{mem::transmute_copy(&self.data_buf)};

        let mut tf =
            Transfer::init_memory_to_peripheral(dup_st, dup_tx, dup_data_buf, None, config);
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
        self.st = st;
        self.tx = tx;
        self.data_buf = data_buf;

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

        let dup_st: StreamX<DMA, S> = unsafe{mem::transmute_copy(&self.st)};
        let dup_tx: Tx<SPI> = unsafe{mem::transmute_copy(&self.tx)};
        let dup_caset_buf: &'static mut [u8; 4] = unsafe{mem::transmute_copy(&self.caset_buf)};

        let mut tf = Transfer::init_memory_to_peripheral(dup_st, dup_tx, dup_caset_buf, None, config);
        tf.start(|_| {});
        tf.wait();
        let (st, tx, caset_buf, _) = tf.release();
        self.st = st;
        self.tx = tx;
        self.caset_buf = caset_buf;
        
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

        let dup_st: StreamX<DMA, S> = unsafe{mem::transmute_copy(&self.st)};
        let dup_tx: Tx<SPI> = unsafe{mem::transmute_copy(&self.tx)};
        let dup_raset_buf: &'static mut [u8; 4] = unsafe{mem::transmute_copy(&self.raset_buf)};

        let mut tf = Transfer::init_memory_to_peripheral(dup_st, dup_tx, dup_raset_buf, None, config);
        tf.start(|_| {});
        tf.wait();
        let (st, tx, raset_buf, _) = tf.release();
        self.st = st;
        self.tx = tx;
        self.raset_buf = raset_buf;
        
        self.d.delay_ms(delay_ms); // Data processing delay
    }

    fn send_data_raw(&mut self, data: &'static [u8]){
        // Raw data send without CS management - for use in chunked transfers
        let config = DmaConfig::default()
            .peripheral_increment(false)
            .memory_increment(true)
            .fifo_enable(false)
            .transfer_complete_interrupt(false);

        let dup_st: StreamX<DMA, S> = unsafe{mem::transmute_copy(&self.st)};
        let dup_tx: Tx<SPI> = unsafe{mem::transmute_copy(&self.tx)};
        let dup_data: &'static mut [u8] = unsafe{mem::transmute_copy(&data)};

        let mut tf = Transfer::init_memory_to_peripheral(dup_st, dup_tx, dup_data, None, config);
        tf.start(|_| {});
        tf.wait();
        let (st, tx, _, _) = tf.release();
        self.st = st;
        self.tx = tx;
    }

    pub fn select(&mut self) -> &mut Self {
        self.cs.set_low().ok(); // Select the device
        self
    }

    pub fn deselect(&mut self) -> &mut Self {
        self.cs.set_high().ok(); // Deselect the device
        self
    }

    // Additional methods for DMA operations can be added here
}
