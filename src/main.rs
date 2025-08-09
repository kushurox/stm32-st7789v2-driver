#![no_std]
#![no_main]

use cortex_m::delay::Delay;
use cortex_m::peripheral::syst::SystClkSource;
use cortex_m::singleton;
use cortex_m_rt::entry;

use defmt::info;
use defmt_rtt as _;
use panic_probe as _;
use stm32f4xx_hal::dma::StreamsTuple;
use stm32f4xx_hal::dwt::DwtExt;
use stm32f4xx_hal::gpio::{self, Speed};
use stm32f4xx_hal::hal::spi::{self, Phase, Polarity};
use stm32f4xx_hal::interrupt;
use stm32f4xx_hal::pac::DWT;
use stm32f4xx_hal::prelude::*;
use stm32f4xx_hal::spi::Spi;
use stm32f4xx_hal::{self, rcc::RccExt};

use crate::st7789v2::ST7789V2DMA;

mod st7789v2;

const W: usize = 240; // Display width
const H: usize = 280; // Display height
const OFFSET: usize = 20; // Non-visible rows at the top

// static BUFFER: &[u8] = include_bytes!("../output.rgb"); // RGB565 data for the display
// static BUFFER: [u8; W * H * 2] = [0xF8; W * H * 2]; // Red pattern in RGB565 format for testing
static BUFFER: &[u8] = include_bytes!("../output.rgb"); // RGB565 data for the display

#[entry]
fn main() -> ! {
    let dp = stm32f4xx_hal::pac::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();

    let mut syst = cp.SYST;

    let rcc = dp.RCC.constrain();

    let clocks = rcc
        .cfgr
        .use_hse(25.MHz())
        .sysclk(48.MHz())
        .hclk(48.MHz())
        .freeze();

    let sfreq = clocks.sysclk().raw();
    let hfreq = clocks.hclk().raw();
    let p1freq = clocks.pclk1().raw();
    let p2freq = clocks.pclk2().raw();

    let src = syst.get_clock_source();

    if let SystClkSource::Core = src {
        info!("using Core clock source"); // rtt is epic
    } else {
        info!("using external clock source");
    }

    info!("sysclk:{}\thclk:{}", sfreq, hfreq);
    info!("pclk1:{}\tpclk2:{}", p1freq, p2freq);

    let pa = dp.GPIOA.split();

    let pa7_mosi = pa
        .pa7
        .into_push_pull_output()
        .speed(Speed::VeryHigh)
        .into_alternate();
    let false_pin = gpio::NoPin::new();
    let pa5_sck = pa
        .pa5
        .into_push_pull_output()
        .speed(Speed::VeryHigh)
        .into_alternate();

    let mode = spi::Mode {
        polarity: Polarity::IdleHigh,
        phase: Phase::CaptureOnSecondTransition,
    };
    let spi = Spi::new(
        dp.SPI1,
        (pa5_sck, false_pin, pa7_mosi),
        mode,
        12.MHz(), // Reduced from 12MHz to 6MHz for large transfers
        &clocks,
    );
    let dc = pa.pa4.into_push_pull_output().speed(Speed::VeryHigh); // high for data and low for command
    let cs = pa.pa3.into_push_pull_output().speed(Speed::VeryHigh);
    let rst = pa.pa2.into_push_pull_output().speed(Speed::VeryHigh);
    let mut d = Delay::new(syst, clocks.hclk().raw());

    let cdwt = cp.DWT.constrain(cp.DCB, &clocks);

    let stream = StreamsTuple::new(dp.DMA2).3;

    let tx = spi.use_dma().tx();
    let cmd_buf = singleton!(: [u8; 1] = [0; 1]).unwrap();
    let data_buf = singleton!(: [u8; 1] = [0; 1]).unwrap();
    let caset_buf = singleton!(: [u8; 4] = [0; 4]).unwrap(); // Column address buffer
    let raset_buf = singleton!(: [u8; 4] = [0; 4]).unwrap(); // Row address buffer
    
    // Create ST7789V2DMA instance with Waveshare-specific configuration:
    // For Waveshare 240x280 display we need:
    // - Width: 240 (standard)
    // - Height: 280 (Waveshare variant, standard ST7789 is 320)  
    // - Offset: 20 (skip 20 non-visible rows at top, standard ST7789 uses 0)
    // The generic parameters are: <'a, SPI, DMA, CS, DC, RST, CHANNEL, S, W, H, OFFSET>
    // All memory buffers are explicitly allocated by user via singleton!
    let mut dma_st: ST7789V2DMA<'_, _, _, _, _, _, 3, 3, W, H, OFFSET> = 
        ST7789V2DMA::new(cs, dc, rst, tx, stream, &mut d, cmd_buf, data_buf, caset_buf, raset_buf);
    dma_st = dma_st.init();
    let t = cdwt.measure(|| {
        dma_st = dma_st.draw_entire_screen(BUFFER);
    });
    
    info!("draw_entire_screen took {} ms", t.as_millis());
    // dma_st.d.delay_ms(3000);
    // dma_st.off();

    loop {}
}

#[interrupt]
fn DMA2_STREAM3() {}
