# 📺 ST7789V2 Display Driver for STM32 (Waveshare 1.69")

This crate provides a driver for the **Waveshare 1.69" LCD Module** (240×280) powered by the **ST7789V2** controller. It is made to work with **STM32 microcontrollers** using the official **HAL crates**, and uses **SPI** to communicate with the display.

---

## 🎯 Project Purpose

This crate aims to make it easy to draw to the ST7789V2-based display using STM32 HAL crates.

It provides:

- A simple way to initialize and control the display
- A function to draw full-screen images using a framebuffer
- Support for color formats the display expects
- Compatibility with common STM32 boards and SPI interfaces

---

## 💡 Target Display

- **Model:** Waveshare 1.69" LCD
- **Resolution:** 240 × 280 pixels
- **Interface:** 4-wire SPI
- **Color Format:** RGB565 (16-bit)

> The display is actually a 240×320 panel internally, but only 240×280 pixels are visible. This driver automatically handles the needed offset.

---

## ✅ Features Done

- [x] Basic initialization of the ST7789V2 controller
- [x] Drawing a full-screen image with a framebuffer
- [x] Column and row addressing handled automatically
- [x] Simple SPI-based communication
- [x] Easy-to-use interface for STM32 HAL users

---

## 📝 TODO

- [ ] Add support for transferring data more efficiently
- [ ] Add support for updating only part of the screen
- [ ] Add support for built-in display features like rotation
- [ ] Add support for drawing shapes or pixels without a full framebuffer
- [ ] Add support for external or user-provided framebuffers

---

## 📦 Example

```rust
const W: usize = 240;
const H: usize = 280;

let mut display = ST7789V2::<_, _, _, _, W, H>::new(spi, dc, rst, cs, &mut delay);
display.init().unwrap();
display.draw_screen(&framebuffer).unwrap();
```