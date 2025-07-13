use cortex_m::delay::Delay;
use stm32f4xx_hal::{hal::{digital::{self, ErrorType, OutputPin}, spi::{self, SpiBus}}, spi::{Instance, Spi}};


struct ST7789V2<'a, SPI, DC, RST, CS>
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

enum Error<SpiE, CSE, DCE>{
    Spi(SpiE),
    CS(CSE),
    DC(DCE)
}

impl<'a, SPI, DC, RST, CS> ST7789V2<'a, SPI, DC, RST, CS>
where
    SPI: Instance,
    DC: OutputPin,
    RST: OutputPin,
    CS: OutputPin
{
    fn new(spi: Spi<SPI>, dc: DC, rst: RST, cs: CS, delay: &'a mut Delay) -> Self {
        Self { spi, dc, rst, cs, delay}
    }

    fn send_command(&mut self, cmd: u8) -> Result<(), Error<stm32f4xx_hal::spi::Error, CS::Error, DC::Error>>{
        self.dc.set_low().map_err(Error::DC)?;
        self.cs.set_low().map_err(Error::CS)?;
        self.spi.write(&[cmd]).map_err(Error::Spi)?;
        self.cs.set_high().map_err(Error::CS)?;

        Ok(())
    }

    fn send_data(&mut self, data: &[u8]) -> Result<(), Error<stm32f4xx_hal::spi::Error, CS::Error, DC::Error>>{
        self.dc.set_high().map_err(Error::DC)?;
        self.cs.set_low().map_err(Error::CS)?;
        self.spi.write(data).map_err(Error::Spi)?;
        self.cs.set_high().map_err(Error::CS)?;

        Ok(())
    }
}