use crate::st7789v2::dma::st7789v2dma::ST7789V2DMA;
use embedded_graphics::{pixelcolor::{raw::ToBytes, Rgb565}, prelude::{Dimensions, DrawTarget, OriginDimensions, Size}};
use stm32f4xx_hal::{
    dma::{
        traits::{Channel, DMASet, Stream}, ChannelX, MemoryToPeripheral, StreamX
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

    fn fill_contiguous<I>(
        &mut self,
        area: &embedded_graphics::primitives::Rectangle,
        colors: I,
    ) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        let drawable_area = area.intersection(&self.bounding_box());
        let (startx, starty) = drawable_area.top_left.into();
        let (width, height) = drawable_area.size.into();
        let endx = startx + width as i32 - 1;
        let endy = starty + height as i32 - 1;

        // Take ownership of the buffer for this call
        let mut chunk_buffer = self.chunk_buffer.take().unwrap();
        let buf_len = chunk_buffer.len();

        let mut idx = 0;

        let mut clrs = colors.into_iter();

        // Prepare LCD for drawing
        self.set_size(startx as u16, endx as u16, starty as u16, endy as u16);
        self.begin_draw();
        self.dc.set_high().ok();
        self.select();

        for _ in 0..(width * height) {
            if idx + 2 > buf_len {
                chunk_buffer = self.send_data_chunk(chunk_buffer);
                idx = 0;
            }
            let color_bytes = clrs.next().unwrap().to_be_bytes();
            chunk_buffer[idx] = color_bytes[0];
            chunk_buffer[idx + 1] = color_bytes[1];
            idx += 2;
        }

        // Flush remaining bytes if needed
        {
            if idx > 0 {
                chunk_buffer = self.send_data_chunk(chunk_buffer);
            };
        }

        self.deselect();

        // Put the buffer back for reuse
        self.chunk_buffer = Some(chunk_buffer);

        Ok(())
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

        unimplemented!("DMA doesnt support drawing individual pixels")

    }
}