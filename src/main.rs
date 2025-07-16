#![no_std]
#![no_main]

use cortex_m::delay::Delay;
use cortex_m::peripheral::syst::SystClkSource;
use cortex_m_rt::entry;

use defmt::println;
use defmt_rtt as _;
use panic_probe as _;
use stm32f4xx_hal::gpio::{self, Speed};
use stm32f4xx_hal::hal::digital::OutputPin;
use stm32f4xx_hal::hal::spi::{self, Phase, Polarity};
use stm32f4xx_hal::pac::dma1::st;
use stm32f4xx_hal::spi::{Instance, Spi};
use stm32f4xx_hal::{self, rcc::RccExt};
use stm32f4xx_hal::prelude::*;
use st7789v2::ST7789V2;

use crate::st7789v2::Commands;


mod st7789v2;

const W : usize = 240; // Width of the display
const H : usize = 280; // Height of the display

static BUFFER: [u8; W*H*2] = [0xEEu8; W * H * 2]; // Buffer for the display, 240x280 pixels, 16 bits per pixel (RGB565)

#[entry]
fn main() -> ! {

    let dp = stm32f4xx_hal::pac::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();

    let mut syst = cp.SYST;

    let rcc = dp.RCC.constrain();

    let clocks = rcc.cfgr
    .use_hse(25.MHz())
    .sysclk(48.MHz())
    .hclk(48.MHz())
    .freeze();


    let sfreq = clocks.sysclk().raw();
    let hfreq = clocks.hclk().raw();

    let src = syst.get_clock_source();

    if let SystClkSource::Core = src {
        println!("using Core clock source");     // rtt is epic
    } else {
        println!("using external clock source");
    }

    println!("sysclk:{}\nhclk:{}\n", sfreq, hfreq);

    let pa = dp.GPIOA.split();

    let pa7_mosi = pa.pa7.into_push_pull_output().speed(Speed::VeryHigh).into_alternate();
    let false_pin = gpio::NoPin::new();
    let pa5_sck = pa.pa5.into_push_pull_output().speed(Speed::VeryHigh).into_alternate();
    
    let mode = spi::Mode {polarity: Polarity::IdleHigh, phase: Phase::CaptureOnSecondTransition};
    let spi = Spi::new(dp.SPI1, (pa5_sck, false_pin, pa7_mosi), mode, 16.MHz(), &clocks);
    let dc = pa.pa4.into_push_pull_output().speed(Speed::High);     // high for data and low for command
    let cs = pa.pa3.into_push_pull_output().speed(Speed::High);
    let rst = pa.pa2.into_push_pull_output().speed(Speed::High);

    let mut d = Delay::new(syst, clocks.hclk().raw());
    let mut st7789v2: ST7789V2<
        _, // SPI type
        _, // DC pin type
        _, // RST pin type
        _, // CS pin type
        W, // width
        H  // height
    > = ST7789V2::new(spi, dc, rst, cs, &mut d);


    st7789v2.init().expect("Failed to initialize ST7789V2 display");
    // manually drawing using send_data and send_command methods for testing from the buffer

    st7789v2.draw_screen(&BUFFER).expect("Failed to draw screen");

    println!("Data sent successfully");



    loop {
    }
}