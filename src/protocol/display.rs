//! Monochrome framebuffer for the two 256x64 Maschine MK2 displays.

use super::device::*;
use super::font;

/// A 256x64 1-bit framebuffer. Pixels are stored page-major (SSD1306 style):
/// `data[page * WIDTH + x]`, where bit `y % 8` of that byte is row `y`.
#[derive(Clone)]
pub struct Framebuffer {
    pub data: [u8; DISPLAY_BYTES],
}

impl Default for Framebuffer {
    fn default() -> Self {
        Framebuffer {
            data: [0; DISPLAY_BYTES],
        }
    }
}

impl Framebuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.data.iter_mut().for_each(|b| *b = 0);
    }

    pub fn fill(&mut self) {
        self.data.iter_mut().for_each(|b| *b = 0xFF);
    }

    #[inline]
    pub fn set_pixel(&mut self, x: i32, y: i32, on: bool) {
        if x < 0 || y < 0 || x as usize >= DISPLAY_WIDTH || y as usize >= DISPLAY_HEIGHT {
            return;
        }
        let (x, y) = (x as usize, y as usize);
        let idx = (y / 8) * DISPLAY_WIDTH + x;
        let bit = 1u8 << (y % 8);
        if on {
            self.data[idx] |= bit;
        } else {
            self.data[idx] &= !bit;
        }
    }

    #[inline]
    pub fn get_pixel(&self, x: usize, y: usize) -> bool {
        if x >= DISPLAY_WIDTH || y >= DISPLAY_HEIGHT {
            return false;
        }
        let idx = (y / 8) * DISPLAY_WIDTH + x;
        (self.data[idx] >> (y % 8)) & 1 == 1
    }

    pub fn hline(&mut self, x: i32, y: i32, len: i32, on: bool) {
        for i in 0..len {
            self.set_pixel(x + i, y, on);
        }
    }

    pub fn vline(&mut self, x: i32, y: i32, len: i32, on: bool) {
        for i in 0..len {
            self.set_pixel(x, y + i, on);
        }
    }

    /// Draw a rectangle outline.
    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, on: bool) {
        self.hline(x, y, w, on);
        self.hline(x, y + h - 1, w, on);
        self.vline(x, y, h, on);
        self.vline(x + w - 1, y, h, on);
    }

    /// Draw a filled rectangle.
    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, on: bool) {
        for row in 0..h {
            self.hline(x, y + row, w, on);
        }
    }

    /// Draw a single character at the top-left position (x, y), scaled by `scale`.
    pub fn draw_char(&mut self, c: char, x: i32, y: i32, scale: i32, on: bool) {
        let scale = scale.max(1);
        for row in 0..font::GLYPH_H {
            for col in 0..font::GLYPH_W {
                if font::pixel(c, col, row) {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            self.set_pixel(
                                x + col as i32 * scale + sx,
                                y + row as i32 * scale + sy,
                                on,
                            );
                        }
                    }
                }
            }
        }
    }

    /// Draw a string. Returns the x position just past the last glyph.
    pub fn draw_text(&mut self, text: &str, x: i32, y: i32, scale: i32) -> i32 {
        let scale = scale.max(1);
        let advance = (font::GLYPH_W as i32 + 1) * scale;
        let mut cx = x;
        for c in text.chars() {
            self.draw_char(c, cx, y, scale, true);
            cx += advance;
        }
        cx
    }

    /// Encode the framebuffer into the 8 display chunks for `display_index`.
    /// Each returned buffer is one chunk: a 9-byte header followed by 256 bytes.
    pub fn encode_chunks(&self, display_index: u8) -> Vec<Vec<u8>> {
        let mut chunks = Vec::with_capacity(DISPLAY_CHUNKS);
        for chunk in 0..DISPLAY_CHUNKS {
            let mut buf = Vec::with_capacity(9 + DISPLAY_CHUNK_BYTES);
            // Header per open-maschine: {0xE0|idx, 0,0, row, 0, 0x20, 0, 0x08, 0}
            // where row = chunk * 8 (the starting pixel row of this 8-px page).
            buf.extend_from_slice(&[
                0xE0 | (display_index & 0x01),
                0x00,
                0x00,
                (chunk * 8) as u8,
                0x00,
                0x20,
                0x00,
                0x08,
                0x00,
            ]);
            let start = chunk * DISPLAY_CHUNK_BYTES;
            buf.extend_from_slice(&self.data[start..start + DISPLAY_CHUNK_BYTES]);
            chunks.push(buf);
        }
        chunks
    }
}
