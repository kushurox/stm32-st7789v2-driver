use cortex_m::delay::Delay;
use defmt::println;
use stm32f4xx_hal::{hal::{digital::{self, ErrorType, OutputPin}, spi::{self, SpiBus}}, spi::{Instance, Spi}};

/// ST7789V2 driver for the ST7789V2 display.
/// This driver uses SPI for communication and requires a data/command pin, a reset pin,
/// and a chip select pin.
/// TODO: Implement DMA support for faster data transfer.
pub struct ST7789V2<'a, SPI, DC, RST, CS, const W: usize, const H: usize, const CMode: u8 = 0x55>
where 
    SPI: Instance,
    DC: OutputPin,
    RST: OutputPin,
    CS: OutputPin
{
    spi: Spi<SPI>,
    dc: DC,
    rst: RST,
    cs: CS,
    delay: &'a mut Delay
}

/// Error type for the ST7789V2 driver.
/// It is a generic error type that can be used to handle errors from the SPI, CS and DC pins.
#[allow(dead_code)]
#[derive(Debug)]
pub enum Error<SpiE, CSE, DCE, RSE>{
    Spi(SpiE),
    CS(CSE),
    DC(DCE),
    RST(RSE),
}


/// Color mode for the ST7789V2 display.
/// This enum defines the color mode used by the display.
/// Currently, only RGB565 (16-bit color mode) is supported.
#[repr(u8)]
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
enum ColorMode {
    RGB565 = 0x55, // 16-bit color mode
}

/// Commands for the ST7789V2 display.
/// This enum defines the commands used to control the display.
/// TODO: Add more commands as needed.
#[repr(u8)]
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum Commands {
    SoftwareReset = 0x01,
    SleepOut = 0x11,
    SetColorMode = 0x3A,
    MemoryDataAccessControl = 0x36,
    DisplayOn = 0x29,
    CASET = 0x2A,
    RASET = 0x2B,
    RAMWR = 0x2C
}


impl<'a, SPI, DC, RST, CS, const W: usize, const H: usize, const CMode: u8> ST7789V2<'a, SPI, DC, RST, CS, W, H, CMode>
where
    SPI: Instance,
    DC: OutputPin,
    RST: OutputPin,
    CS: OutputPin
{

    pub const BUFFER_SIZE: usize = W * H * 2; // 2 bytes per pixel for RGB565

    /// Creates a new instance of the ST7789V2 driver.
    /// # Arguments
    /// * `spi` - The SPI interface to use for communication. must be initialized.
    /// * `dc` - The data/command pin, used to switch between data and command mode. when high, it is in data mode and when low, it is in command mode.
    /// * `rst` - The reset pin, used to reset the display.
    /// * `cs` - The chip select pin, used to select the display. it is active low.
    /// * `delay` - A mutable reference to a delay object, used for timing operations.
    /// # Returns
    /// A new instance of the ST7789V2 driver.
    pub const fn new(spi: Spi<SPI>, dc: DC, rst: RST, cs: CS, delay: &'a mut Delay) -> Self {
        // initialzing the controller
        Self { spi, dc, rst, cs, delay}
    }

    /// Initializes the ST7789V2 display.
    /// This method sends the initialization commands in the order of
    /// 1. Software reset
    /// 2. Sleep out
    /// 3. Set color mode
    /// 4. Memory data access control
    /// 5. Display on
    /// # Returns
    /// A result indicating success or failure of the initialization.
    /// note: that this method will block until the display is initialized.
    /// note: there is a delay after each command to allow the display to process the command.
    pub fn init(&mut self) -> Result<(), Error<stm32f4xx_hal::spi::Error, CS::Error, DC::Error, RST::Error>> {
        // Reset the display
        self.rst.set_low().map_err(Error::RST)?;
        self.delay.delay_ms(120);
        self.rst.set_high().map_err(Error::RST)?;
        self.delay.delay_ms(150);

        // Initialization sequence for ST7789V2
        self.send_command(Commands::SoftwareReset)?; // Software reset
        self.delay.delay_ms(150);
        self.send_command(Commands::SleepOut)?; // Sleep out
        self.delay.delay_ms(150);

        self.send_command(Commands::SetColorMode)?; // Set color mode
        self.send_data(&[CMode])?; // Set to RGB565 color mode
        self.delay.delay_ms(10);

        self.send_command(Commands::MemoryDataAccessControl)?; // Memory data access control
        self.send_data(&[0x00])?; // Set to normal mode (no rotation)
        self.delay.delay_ms(10);

        self.send_command(Commands::DisplayOn)?; // Display on
        self.delay.delay_ms(10);

        // Other initialization commands can be added here

        Ok(())
    }


    /// Draws the screen with the provided buffer. uses W and H constants to determine the column address and row address.
    pub fn draw_screen(&mut self, buffer: &[u8]) -> Result<(), Error<stm32f4xx_hal::spi::Error, CS::Error, DC::Error, RST::Error>> {

        let (ca_msb, ca_lsb) = ((W >> 8) as u8, (W & 0xFF) as u8);
        let (ra_msb, ra_lsb) = ((H >> 8) as u8, (H & 0xFF) as u8);
        // Set the column address
        self.send_command(Commands::CASET)?;
        self.send_data(&[0x00, 0x00, ca_msb, ca_lsb])?;

        println!("set column address: 0x00 0x00 0x{:02X} 0x{:02X}", ca_msb, ca_lsb);

        // Set the row address
        self.send_command(Commands::RASET)?;
        self.send_data(&[0x00, 0x00, ra_msb, ra_lsb])?;

        println!("set row address: 0x00 0x00 0x{:02X} 0x{:02X}", ra_msb, ra_lsb);

        // Write memory
        self.send_command(Commands::RAMWR)?;
        self.send_data(buffer)?;

        println!("draw screen with buffer of size: {}", buffer.len());

        Ok(())
    }

    pub fn send_command(&mut self, cmd: Commands) -> Result<(), Error<stm32f4xx_hal::spi::Error, CS::Error, DC::Error, RST::Error>> {
        self.dc.set_low().map_err(Error::DC)?;
        self.cs.set_low().map_err(Error::CS)?;
        self.spi.write(&[cmd as u8]).map_err(Error::Spi)?;
        self.cs.set_high().map_err(Error::CS)?;

        Ok(())
    }

    pub fn send_data(&mut self, data: &[u8]) -> Result<(), Error<stm32f4xx_hal::spi::Error, CS::Error, DC::Error, RST::Error>> {
        self.dc.set_high().map_err(Error::DC)?;
        self.cs.set_low().map_err(Error::CS)?;
        self.spi.write(data).map_err(Error::Spi)?;
        self.cs.set_high().map_err(Error::CS)?;

        Ok(())
    }
}