//! 8-bit RGB color used for pad and group LEDs.

/// A simple 24-bit RGB color. The Maschine MK2 drives each RGB LED with three
/// consecutive bytes (red, green, blue).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const BLACK: Color = Color { r: 0, g: 0, b: 0 };
    pub const WHITE: Color = Color {
        r: 255,
        g: 255,
        b: 255,
    };
    pub const RED: Color = Color { r: 255, g: 0, b: 0 };
    pub const GREEN: Color = Color { r: 0, g: 255, b: 0 };
    pub const BLUE: Color = Color { r: 0, g: 0, b: 255 };
    pub const ORANGE: Color = Color {
        r: 255,
        g: 80,
        b: 0,
    };
    pub const CYAN: Color = Color {
        r: 0,
        g: 200,
        b: 200,
    };
    pub const PURPLE: Color = Color {
        r: 160,
        g: 0,
        b: 200,
    };
    pub const YELLOW: Color = Color {
        r: 255,
        g: 200,
        b: 0,
    };

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b }
    }

    /// Scale brightness by `factor` in [0.0, 1.0].
    pub fn dim(self, factor: f32) -> Color {
        let f = factor.clamp(0.0, 1.0);
        Color {
            r: (self.r as f32 * f) as u8,
            g: (self.g as f32 * f) as u8,
            b: (self.b as f32 * f) as u8,
        }
    }
}
