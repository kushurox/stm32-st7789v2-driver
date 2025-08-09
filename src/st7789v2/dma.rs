use crate::st7789v2::common::{ColorMode, Commands};
use core::ptr::{addr_of_mut, read_volatile};
use core::sync::atomic::{Ordering, compiler_fence};
use cortex_m::delay::Delay;
use defmt::{debug, info};
use stm32f4xx_hal::{
    ReadFlags,
    dma::{
        ChannelX, MemoryToPeripheral, StreamX, Transfer,
        config::DmaConfig,
        traits::{Channel, DMASet, Stream, StreamISR},
    },
    hal::digital::OutputPin,
    rcc,
    spi::{Instance, Tx},
};

// Static buffers for DMA transfers - these need to be global for the 'static lifetime
static mut CASET_DATA: [u8; 4] = [0; 4]; // Column address set data buffer
static mut RASET_DATA: [u8; 4] = [0; 4]; // Row address set data buffer

// Macro for handling CS timing with commands
macro_rules! cs_command {
    ($self:expr, $cmd:expr, $delay_ms:expr) => {{
        $self.cs.set_low().ok(); // Select device
        $self = $self.send_command($cmd); // Send command (CS stays low)
        $self.d.delay_ms($delay_ms); // Delay while CS is still low for processing
        $self.cs.set_high().ok(); // Deselect device after delay
        $self
    }};
}

// Macro for handling CS timing with data
macro_rules! cs_data {
    ($self:expr, $data:expr, $delay_ms:expr) => {{
        $self.cs.set_low().ok(); // Select device
        $self = $self.send_data_u8($data); // Send data (CS stays low)
        $self.d.delay_ms($delay_ms); // Delay while CS is still low for processing
        $self.cs.set_high().ok(); // Deselect device after delay
        $self
    }};
}

// Macro for handling CS timing with data arrays
macro_rules! cs_data_array {
    ($self:expr, $data:expr, $delay_ms:expr) => {{
        $self.cs.set_low().ok(); // Select device
        $self = $self.send_data($data); // Send data array (CS stays low)
        $self.d.delay_ms($delay_ms); // Delay while CS is still low for processing
        $self.cs.set_high().ok(); // Deselect device after delay
        $self
    }};
}

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
    const H: usize = 320,
    const OFFSET: usize = 0,
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
        }
    }

    pub fn init(mut self) -> Self {
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
        self = cs_command!(self, Commands::SoftwareReset, 150);
        debug!("Software reset step completed in init()");

        self = cs_command!(self, Commands::SleepOut, 120);
        debug!("Sleep out step completed in init()");

        self = cs_command!(self, Commands::SetColorMode, 1);
        self = cs_data!(self, ColorMode::RGB565 as u8, 10);
        debug!("Set color mode step completed in init()");

        self = cs_command!(self, Commands::MemoryDataAccessControl, 1);
        self = cs_data!(self, 0b0000_0000, 10); // Set to normal mode (no rotation)
        debug!("Memory data access control step completed in init()");

        self = cs_command!(self, Commands::InversionOn, 1);
        debug!("Inversion on step completed in init()");

        self = cs_command!(self, Commands::DisplayOn, 50);
        debug!("Display on step completed in init()");

        self
    }

    pub fn draw_entire_screen(mut self, buffer: &'static [u8]) -> Self {
        // Display has OFFSET non-visible rows at top and bottom
        // So visible area is from row OFFSET to row (OFFSET + H - 1)
        let x_start = 0u16; // Start at column 0
        let x_end = W as u16 - 1; // End at column (W-1)

        let y_start = OFFSET as u16; // Start at row OFFSET (skip first OFFSET non-visible rows)
        let y_end = y_start + H as u16 - 1; // End at row (OFFSET + H - 1)

        // Prepare CASET data (Column Address Set)
        let caset_data = unsafe { &mut *addr_of_mut!(CASET_DATA) };
        caset_data[0] = (x_start >> 8) as u8; // Start column MSB
        caset_data[1] = (x_start & 0xFF) as u8; // Start column LSB
        caset_data[2] = (x_end >> 8) as u8; // End column MSB
        caset_data[3] = (x_end & 0xFF) as u8; // End column LSB

        // Prepare RASET data (Row Address Set)
        let raset_data = unsafe { &mut *addr_of_mut!(RASET_DATA) };
        raset_data[0] = (y_start >> 8) as u8; // Start row MSB
        raset_data[1] = (y_start & 0xFF) as u8; // Start row LSB
        raset_data[2] = (y_end >> 8) as u8; // End row MSB
        raset_data[3] = (y_end & 0xFF) as u8; // End row LSB

        debug!(
            "Drawing {}x{} image at position ({},{}) to ({},{}) with offset {}",
            W, H, x_start, y_start, x_end, y_end, OFFSET
        );

        // Send commands and address setup using macros for proper CS timing
        self = cs_command!(self, Commands::CASET, 1);
        self = cs_data_array!(self, caset_data, 1);
        debug!("Column address set command sent");

        self = cs_command!(self, Commands::RASET, 1);
        self = cs_data_array!(self, raset_data, 1);
        debug!("Row address set command sent");

        self = cs_command!(self, Commands::RAMWR, 10);

        // Now send the image data in chunks with CS low for entire transfer (like working code)
        self.dc.set_high().ok(); // Set data mode for image data
        self.cs.set_low().ok(); // Select device for entire transfer

        let chunk_size = 64 * 1024; // 64KB chunks

        for chunk in buffer.chunks(chunk_size) {
            self = self.send_data_raw(chunk);
        }

        self.cs.set_high().ok(); // Deselect device after all chunks

        self
    }

    pub fn off(mut self) -> Self {
        cs_command!(self, Commands::DisplayOff, 50)
    }

    fn send_command(mut self, cmd: Commands) -> Self {
        self.cmd_buf[0] = cmd as u8;

        // Set DC mode (CS is handled externally by macro)
        self.dc.set_low().ok(); // Command mode

        let config = DmaConfig::default()
            .peripheral_increment(false)
            .memory_increment(true)
            .fifo_enable(false)
            .transfer_complete_interrupt(false);
        let mut tf =
            Transfer::init_memory_to_peripheral(self.st, self.tx, self.cmd_buf, None, config);
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
        self
    }

    fn send_data_u8(mut self, data: u8) -> Self {
        self.data_buf[0] = data;

        // Set DC mode (CS is handled externally by macro)
        self.dc.set_high().ok(); // Data mode

        let config = DmaConfig::default()
            .peripheral_increment(false)
            .memory_increment(true)
            .fifo_enable(false)
            .transfer_complete_interrupt(false);
        let mut tf =
            Transfer::init_memory_to_peripheral(self.st, self.tx, self.data_buf, None, config);
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
        self
    }

    fn send_data(mut self, data: &'static [u8]) -> Self {
        self.dc.set_high().ok();
        let config = DmaConfig::default()
            .peripheral_increment(false)
            .memory_increment(true)
            .fifo_enable(false)
            .transfer_complete_interrupt(false);
        let mut tf = Transfer::init_memory_to_peripheral(self.st, self.tx, data, None, config);
        tf.start(|_| {});
        tf.wait();
        let (st, tx, _, _) = tf.release();
        self.st = st;
        self.tx = tx;
        self
    }

    fn send_data_raw(mut self, data: &'static [u8]) -> Self {
        // Raw data send without CS management - for use in chunked transfers
        let config = DmaConfig::default()
            .peripheral_increment(false)
            .memory_increment(true)
            .fifo_enable(false)
            .transfer_complete_interrupt(false);
        let mut tf = Transfer::init_memory_to_peripheral(self.st, self.tx, data, None, config);
        tf.start(|_| {});
        tf.wait();
        let (st, tx, _, _) = tf.release();
        self.st = st;
        self.tx = tx;
        self
    }
    // Additional methods for DMA operations can be added here
}
