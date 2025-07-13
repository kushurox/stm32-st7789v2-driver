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
use stm32f4xx_hal::spi::{Instance, Spi};
use stm32f4xx_hal::{self, rcc::RccExt};
use stm32f4xx_hal::prelude::*;


mod st7789v2;



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

    
    let buffer = [0u8; 512];

    let pa = dp.GPIOA.split();

    let pa7_mosi = pa.pa7.into_push_pull_output().speed(Speed::VeryHigh).into_alternate();
    let false_pin = gpio::NoPin::new();
    let pa5_sck = pa.pa5.into_push_pull_output().speed(Speed::VeryHigh).into_alternate();
    
    let mode = spi::Mode {polarity: Polarity::IdleHigh, phase: Phase::CaptureOnSecondTransition};
    let spi = Spi::new(dp.SPI1, (pa5_sck, false_pin, pa7_mosi), mode, 16.MHz(), &clocks);

    let buffer = [0u8; 512];
    let dc = pa.pa4.into_push_pull_output().speed(Speed::High);     // high for data and low for command
    let cs = pa.pa3.into_push_pull_output().speed(Speed::High);
    let rst = pa.pa2.into_push_pull_output().speed(Speed::High);



    loop {
    }
}