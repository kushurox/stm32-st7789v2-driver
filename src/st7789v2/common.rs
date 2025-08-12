
/// Error type for the ST7789V2 driver.
/// It is a generic error type that can be used to handle errors from the SPI, CS and DC pins.
#[allow(dead_code)]
#[derive(Debug)]
pub enum Error<SpiE, CSE, DCE, RSE> {
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
pub enum ColorMode {
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
    DisplayOff = 0x28,
    CASET = 0x2A,
    RASET = 0x2B,
    RAMWR = 0x2C,
    InversionOn = 0x21,
    InversionOff = 0x20,
}
