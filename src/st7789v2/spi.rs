use crate::st7789v2::common::{Commands, Error};
use cortex_m::delay::Delay;
use defmt::debug;
use stm32f4xx_hal::{
    hal::digital::OutputPin, spi::{Instance, Spi}
};

/// ST7789V2 driver for the ST7789V2 display.
/// This driver uses SPI for communication and requires a data/command pin, a reset pin,
/// and a chip select pin.
/// TODO: Implement DMA support for faster data transfer.
pub struct ST7789V2<'a, SPI, DC, RST, CS, const W: usize, const H: usize>
where
    SPI: Instance,
    DC: OutputPin,
    RST: OutputPin,
    CS: OutputPin,
{
    spi: Spi<SPI>,
    dc: DC,
    rst: RST,
    cs: CS,
    delay: &'a mut Delay,
}

impl<'a, SPI, DC, RST, CS, const W: usize, const H: usize> ST7789V2<'a, SPI, DC, RST, CS, W, H>
where
    SPI: Instance,
    DC: OutputPin,
    RST: OutputPin,
    CS: OutputPin,
{
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
        Self {
            spi,
            dc,
            rst,
            cs,
            delay,
        }
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
    pub fn init(
        &mut self,
    ) -> Result<(), Error<stm32f4xx_hal::spi::Error, CS::Error, DC::Error, RST::Error>> {
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
        self.send_data(&[0x55])?; // Set to RGB565 color mode
        self.delay.delay_ms(10);

        self.send_command(Commands::MemoryDataAccessControl)?; // Memory data access control
        self.send_data(&[0b0000_0000])?; // Set to normal mode (no rotation)
        self.delay.delay_ms(10);

        self.send_command(Commands::DisplayOn)?; // Display on
        self.delay.delay_ms(10);

        // Other initialization commands can be added here

        Ok(())
    }

    /// Draws the screen with the provided buffer. uses W and H constants to determine the column address and row address.
    pub fn draw_screen(
        &mut self,
        buffer: &[u8],
    ) -> Result<(), Error<stm32f4xx_hal::spi::Error, CS::Error, DC::Error, RST::Error>> {
        let y_offset = 20; // Y offset for the display
        let y_end = y_offset + H as u16 - 1; // Y end address for the display

        let x_offset = 0; // X offset for the display
        let x_end = W as u16 - 1; // X end address for the

        let ra_start_msb = (y_offset >> 8) as u8; // Row address start MSB
        let ra_start_lsb = (y_offset & 0xFF) as u8; // Row address start LSB
        let ra_end_msb = (y_end >> 8) as u8; // Row address end MSB
        let ra_end_lsb = (y_end & 0xFF) as u8; // Row address end LSB

        let ca_start_msb = (x_offset >> 8) as u8; // Column address start MSB
        let ca_start_lsb = (x_offset & 0xFF) as u8; // Column address start LSB
        let ca_end_msb = (x_end >> 8) as u8; // Column address end MSB
        let ca_end_lsb = (x_end & 0xFF) as u8; // Column address end LSB

        // Set the column address
        self.send_command(Commands::CASET)?;
        self.send_data(&[ca_start_msb, ca_start_lsb, ca_end_msb, ca_end_lsb])?;

        debug!(
            "set column address: 0x{:02X} 0x{:02X} 0x{:02X} 0x{:02X}",
            ca_start_msb, ca_start_lsb, ca_end_msb, ca_end_lsb
        );

        // Set the row address
        self.send_command(Commands::RASET)?;
        self.send_data(&[ra_start_msb, ra_start_lsb, ra_end_msb, ra_end_lsb])?;

        debug!(
            "set row address: 0x{:02X} 0x{:02X} 0x{:02X} 0x{:02X}",
            ra_start_msb, ra_start_lsb, ra_end_msb, ra_end_lsb
        );

        // Write memory
        self.send_command(Commands::RAMWR)?;
        self.send_data(buffer)?;

        debug!("draw screen with buffer of size: {}", buffer.len());

        Ok(())
    }

    pub fn send_command(
        &mut self,
        cmd: Commands,
    ) -> Result<(), Error<stm32f4xx_hal::spi::Error, CS::Error, DC::Error, RST::Error>> {
        self.dc.set_low().map_err(Error::DC)?;
        self.cs.set_low().map_err(Error::CS)?;
        self.spi.write(&[cmd as u8]).map_err(Error::Spi)?;
        self.cs.set_high().map_err(Error::CS)?;

        Ok(())
    }

    pub fn send_data(
        &mut self,
        data: &[u8],
    ) -> Result<(), Error<stm32f4xx_hal::spi::Error, CS::Error, DC::Error, RST::Error>> {
        self.dc.set_high().map_err(Error::DC)?;
        self.cs.set_low().map_err(Error::CS)?;
        self.spi.write(data).map_err(Error::Spi)?;
        self.cs.set_high().map_err(Error::CS)?;

        Ok(())
    }

    pub fn release(self) -> (Spi<SPI>, DC, RST, CS) {
        // Release the resources held by the driver
        (self.spi, self.dc, self.rst, self.cs)
    }
}
