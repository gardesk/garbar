use cairo::{Context, LinearGradient, RadialGradient};

use super::Color;

#[derive(Debug, Clone)]
pub struct GradientStop {
    pub position: f64, // 0.0 to 1.0
    pub color: Color,
}

impl GradientStop {
    pub fn new(position: f64, color: Color) -> Self {
        Self { position, color }
    }
}

#[derive(Debug, Clone)]
pub enum GradientDirection {
    Horizontal,
    Vertical,
    Diagonal,    // Top-left to bottom-right
    DiagonalRev, // Top-right to bottom-left
    Custom { start: (f64, f64), end: (f64, f64) },
}

#[derive(Debug, Clone)]
pub enum Gradient {
    Linear {
        direction: GradientDirection,
        stops: Vec<GradientStop>,
    },
    Radial {
        center: (f64, f64), // Normalized 0.0-1.0
        radius: f64,        // Normalized to smaller dimension
        stops: Vec<GradientStop>,
    },
}

impl Gradient {
    pub fn horizontal(stops: Vec<GradientStop>) -> Self {
        Self::Linear {
            direction: GradientDirection::Horizontal,
            stops,
        }
    }

    pub fn vertical(stops: Vec<GradientStop>) -> Self {
        Self::Linear {
            direction: GradientDirection::Vertical,
            stops,
        }
    }

    pub fn radial(center: (f64, f64), radius: f64, stops: Vec<GradientStop>) -> Self {
        Self::Radial { center, radius, stops }
    }

    /// Apply gradient to Cairo context for a given rectangle
    pub fn apply(&self, cr: &Context, x: f64, y: f64, width: f64, height: f64) {
        match self {
            Self::Linear { direction, stops } => {
                let (x0, y0, x1, y1) = match direction {
                    GradientDirection::Horizontal => (x, y, x + width, y),
                    GradientDirection::Vertical => (x, y, x, y + height),
                    GradientDirection::Diagonal => (x, y, x + width, y + height),
                    GradientDirection::DiagonalRev => (x + width, y, x, y + height),
                    GradientDirection::Custom { start, end } => (
                        x + start.0 * width,
                        y + start.1 * height,
                        x + end.0 * width,
                        y + end.1 * height,
                    ),
                };

                let gradient = LinearGradient::new(x0, y0, x1, y1);
                for stop in stops {
                    gradient.add_color_stop_rgba(
                        stop.position,
                        stop.color.r,
                        stop.color.g,
                        stop.color.b,
                        stop.color.a,
                    );
                }
                cr.set_source(&gradient).unwrap();
            }
            Self::Radial { center, radius, stops } => {
                let cx = x + center.0 * width;
                let cy = y + center.1 * height;
                let r = radius * width.min(height);

                let gradient = RadialGradient::new(cx, cy, 0.0, cx, cy, r);
                for stop in stops {
                    gradient.add_color_stop_rgba(
                        stop.position,
                        stop.color.r,
                        stop.color.g,
                        stop.color.b,
                        stop.color.a,
                    );
                }
                cr.set_source(&gradient).unwrap();
            }
        }
    }
}

/// Background can be either a solid color, gradient, or transparent
#[derive(Debug, Clone)]
pub enum Background {
    None,
    Solid(Color),
    Gradient(Gradient),
}

impl Background {
    /// Check if this background is transparent/none
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    pub fn apply(&self, cr: &Context, x: f64, y: f64, width: f64, height: f64) {
        match self {
            Self::None => {} // Don't set any source - nothing to draw
            Self::Solid(color) => color.apply(cr),
            Self::Gradient(gradient) => gradient.apply(cr, x, y, width, height),
        }
    }
}

impl From<Color> for Background {
    fn from(color: Color) -> Self {
        Self::Solid(color)
    }
}

impl From<Gradient> for Background {
    fn from(gradient: Gradient) -> Self {
        Self::Gradient(gradient)
    }
}

impl Default for Background {
    fn default() -> Self {
        Self::None
    }
}
