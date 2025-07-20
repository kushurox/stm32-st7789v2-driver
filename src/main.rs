#![no_std]
#![no_main]


use cortex_m::delay::Delay;
use cortex_m::peripheral::syst::SystClkSource;
use cortex_m_rt::entry;

use defmt::info;
use defmt_rtt as _;
use embedded_graphics::pixelcolor::Rgb565;
use panic_probe as _;
use st7789v2::ST7789V2;
use stm32f4xx_hal::dma::config::DmaConfig;
use stm32f4xx_hal::dma::{StreamsTuple, Transfer};
use stm32f4xx_hal::gpio::{self, Speed};
use stm32f4xx_hal::hal::spi::{self, Phase, Polarity};
use stm32f4xx_hal::hal_02::blocking::spi::transfer;
use stm32f4xx_hal::pac::sdio::fifo;
use stm32f4xx_hal::{block, prelude::*};
use stm32f4xx_hal::spi::Spi;
use stm32f4xx_hal::{self, rcc::RccExt};
use stm32f4xx_hal::{pac::interrupt};

use tinybmp::Bmp;
use crate::st7789v2::Commands;

mod st7789v2;

const W: usize = 240; // Width of the display
const H: usize = 280; // Height of the display

static BUFFER: &[u8] = include_bytes!("../output.rgb"); // RGB565 data for the display
// static BUFFER: [u8; W * H * 2] = [0xFAu8; W * H * 2]; // test black image

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
        16.MHz(),
        &clocks,
    );
    let dc = pa.pa4.into_push_pull_output().speed(Speed::VeryHigh); // high for data and low for command
    let cs = pa.pa3.into_push_pull_output().speed(Speed::VeryHigh);
    let rst = pa.pa2.into_push_pull_output().speed(Speed::VeryHigh);
    let mut d = Delay::new(syst, clocks.hclk().raw());


    let mut display: ST7789V2<'_, _, _, _, _, W, H> = ST7789V2::new(spi, dc, rst, cs, &mut d);
    display.init().unwrap();

    display.send_command(Commands::CASET);
    display.send_data(&[0x00, 0x00, 0x00, 0xEF]).unwrap(); // Column address set
    display.send_command(Commands::RASET);
    display.send_data(&[0x00, 0x00, 0x01, 0x3F]).unwrap(); // Row address set
    display.send_command(Commands::RAMWR).unwrap(); // Write to RAM

    let (spi, dc, rst, cs) = display.release(); // Release the resources held by the driver for DMA transfer

    let streams = StreamsTuple::new(dp.DMA2);
    let stream_tx = streams.3; // DMA stream for TX


    let tx = spi.use_dma().tx();

    let congif = DmaConfig::default()
        .memory_increment(true)
        .peripheral_increment(false)
        .fifo_enable(true)
        .fifo_error_interrupt(false)
        .transfer_complete_interrupt(false)
        .half_transfer_interrupt(false)
        .transfer_error_interrupt(false);

    let mut dma = Transfer::init_memory_to_peripheral(stream_tx, tx, BUFFER, None, congif);

    dma.start(|_| {
        info!("DMA transfer started");
    });


    loop {
        dma.is_transfer_complete().then(|| info!("transfer complete"));

    }
}
