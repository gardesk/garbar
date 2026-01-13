use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

#[derive(Debug, Error)]
pub enum ColorError {
    #[error("invalid hex color format: {0}")]
    InvalidHex(String),
    #[error("invalid rgba format: {0}")]
    InvalidRgba(String),
}

impl Color {
    pub const fn new(r: f64, g: f64, b: f64, a: f64) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: f64, g: f64, b: f64) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub const fn transparent() -> Self {
        Self { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }
    }

    pub const fn black() -> Self {
        Self::rgb(0.0, 0.0, 0.0)
    }

    pub const fn white() -> Self {
        Self::rgb(1.0, 1.0, 1.0)
    }

    /// Parse hex color: #RGB, #RGBA, #RRGGBB, #RRGGBBAA
    pub fn from_hex(s: &str) -> Result<Self, ColorError> {
        let s = s.trim_start_matches('#');

        let (r, g, b, a) = match s.len() {
            3 => {
                // #RGB
                let r = u8::from_str_radix(&s[0..1], 16).map_err(|_| ColorError::InvalidHex(s.to_string()))?;
                let g = u8::from_str_radix(&s[1..2], 16).map_err(|_| ColorError::InvalidHex(s.to_string()))?;
                let b = u8::from_str_radix(&s[2..3], 16).map_err(|_| ColorError::InvalidHex(s.to_string()))?;
                // Expand: F -> FF
                ((r << 4) | r, (g << 4) | g, (b << 4) | b, 255u8)
            }
            4 => {
                // #RGBA
                let r = u8::from_str_radix(&s[0..1], 16).map_err(|_| ColorError::InvalidHex(s.to_string()))?;
                let g = u8::from_str_radix(&s[1..2], 16).map_err(|_| ColorError::InvalidHex(s.to_string()))?;
                let b = u8::from_str_radix(&s[2..3], 16).map_err(|_| ColorError::InvalidHex(s.to_string()))?;
                let a = u8::from_str_radix(&s[3..4], 16).map_err(|_| ColorError::InvalidHex(s.to_string()))?;
                ((r << 4) | r, (g << 4) | g, (b << 4) | b, (a << 4) | a)
            }
            6 => {
                // #RRGGBB
                let r = u8::from_str_radix(&s[0..2], 16).map_err(|_| ColorError::InvalidHex(s.to_string()))?;
                let g = u8::from_str_radix(&s[2..4], 16).map_err(|_| ColorError::InvalidHex(s.to_string()))?;
                let b = u8::from_str_radix(&s[4..6], 16).map_err(|_| ColorError::InvalidHex(s.to_string()))?;
                (r, g, b, 255u8)
            }
            8 => {
                // #RRGGBBAA
                let r = u8::from_str_radix(&s[0..2], 16).map_err(|_| ColorError::InvalidHex(s.to_string()))?;
                let g = u8::from_str_radix(&s[2..4], 16).map_err(|_| ColorError::InvalidHex(s.to_string()))?;
                let b = u8::from_str_radix(&s[4..6], 16).map_err(|_| ColorError::InvalidHex(s.to_string()))?;
                let a = u8::from_str_radix(&s[6..8], 16).map_err(|_| ColorError::InvalidHex(s.to_string()))?;
                (r, g, b, a)
            }
            _ => return Err(ColorError::InvalidHex(s.to_string())),
        };

        Ok(Self {
            r: r as f64 / 255.0,
            g: g as f64 / 255.0,
            b: b as f64 / 255.0,
            a: a as f64 / 255.0,
        })
    }

    /// Apply color to Cairo context
    pub fn apply(&self, cr: &cairo::Context) {
        cr.set_source_rgba(self.r, self.g, self.b, self.a);
    }

    /// Interpolate between two colors
    pub fn lerp(&self, other: &Color, t: f64) -> Color {
        Color {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }

    /// Convert to u32 (ARGB format for X11)
    pub fn to_argb_u32(&self) -> u32 {
        let a = (self.a * 255.0) as u32;
        let r = (self.r * 255.0) as u32;
        let g = (self.g * 255.0) as u32;
        let b = (self.b * 255.0) as u32;
        (a << 24) | (r << 16) | (g << 8) | b
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::black()
    }
}

impl FromStr for Color {
    type Err = ColorError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();

        if s == "transparent" {
            return Ok(Self::transparent());
        }

        if s.starts_with('#') || s.chars().all(|c| c.is_ascii_hexdigit()) {
            return Self::from_hex(s);
        }

        // Named colors
        match s.to_lowercase().as_str() {
            "black" => Ok(Self::black()),
            "white" => Ok(Self::white()),
            "red" => Ok(Self::rgb(1.0, 0.0, 0.0)),
            "green" => Ok(Self::rgb(0.0, 1.0, 0.0)),
            "blue" => Ok(Self::rgb(0.0, 0.0, 1.0)),
            "yellow" => Ok(Self::rgb(1.0, 1.0, 0.0)),
            "cyan" => Ok(Self::rgb(0.0, 1.0, 1.0)),
            "magenta" => Ok(Self::rgb(1.0, 0.0, 1.0)),
            _ => Self::from_hex(s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_parsing() {
        let c = Color::from_hex("#ff0000").unwrap();
        assert!((c.r - 1.0).abs() < 0.001);
        assert!((c.g - 0.0).abs() < 0.001);
        assert!((c.b - 0.0).abs() < 0.001);

        let c = Color::from_hex("#00ff0080").unwrap();
        assert!((c.g - 1.0).abs() < 0.001);
        assert!((c.a - 0.502).abs() < 0.01);
    }

    #[test]
    fn test_short_hex() {
        let c = Color::from_hex("#f00").unwrap();
        assert!((c.r - 1.0).abs() < 0.001);
    }
}
