

use core::{ops::{Not, Rem}, u32};

use crate::st7789v2::dma::st7789v2dma::ST7789V2DMA;
use embedded_graphics::{pixelcolor::{raw::ToBytes, Rgb565}, prelude::{Dimensions, DrawTarget, OriginDimensions, Point, PointsIter, Size}, Pixel};
use stm32f4xx_hal::{
    dma::{
        ChannelX, MemoryToPeripheral, StreamX,
        traits::{Channel, DMASet, Stream},
    },
    hal::digital::OutputPin,
    rcc,
    spi::Instance,
};

impl<'a, SPI, DMA, CS, DC, RST, const CHANNEL: u8, const S: u8, const W: usize, const H: usize, const OFFSET: usize> OriginDimensions for
    ST7789V2DMA<'a, SPI, DMA, CS, DC, RST, CHANNEL, S, W, H, OFFSET>
where
    SPI: Instance + DMASet<StreamX<DMA, S>, CHANNEL, MemoryToPeripheral>,
    CS: OutputPin,
    DC: OutputPin,
    RST: OutputPin,
    DMA: rcc::Enable + rcc::Reset + stm32f4xx_hal::dma::traits::Instance,
    StreamX<DMA, S>: Stream,
    ChannelX<CHANNEL>: Channel
{
    fn size(&self) -> embedded_graphics::prelude::Size {
        Size::new(W as u32, H as u32)
    }
}


impl<'a, SPI, DMA, CS, DC, RST, const CHANNEL: u8, const S: u8, const W: usize, const H: usize, const OFFSET: usize> DrawTarget for
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

    type Color = Rgb565;
    type Error = core::convert::Infallible;

    fn fill_contiguous<I>(&mut self, area: &embedded_graphics::primitives::Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        self.draw_iter(
            area.points()
                .zip(colors)
                .map(|(pos, color)| embedded_graphics::Pixel(pos, color)),
        )
    }

    fn fill_solid(&mut self, area: &embedded_graphics::primitives::Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        self.fill_contiguous(area, core::iter::repeat(color))
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.fill_solid(&self.bounding_box(), color)
    }
    
    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>> {
        let mut framebuf: [u8; 4 * 1024] = [0u8; 4 * 1024]; // 4KB frame buffer
        // since each pix is 16 bit, there are 2048 pixels
        let row_offset = W as i32 * 2;
        for Pixel(p, clr) in pixels.into_iter() {
            if !(0..W as i32).contains(&p.x) || !(0..H as i32).contains(&p.y) {
                continue;
            }
            let mut idx: usize = ((p.x * 2) + (p.y * row_offset)) as usize;
            if idx >= framebuf.len() {
                // we have hit a chunk worth of data
                
            }
            idx = idx.rem(framebuf.len()/ 2); // because we are sending in chunks of 4 KiB
            let color = clr.to_be_bytes();
            framebuf[idx] = color[0];
            framebuf[idx + 1] = color[1];
        }

        Ok(())
    }
}