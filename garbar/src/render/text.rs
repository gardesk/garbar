use anyhow::Result;
use cairo::Context;
use pango::{FontDescription, Layout, EllipsizeMode, Alignment as PangoAlignment};
use pangocairo::functions as pangocairo;
use tracing::debug;

use super::Color;

/// Text renderer using Pango for full Unicode support
pub struct TextRenderer {
    fonts: Vec<FontDescription>,
}

impl TextRenderer {
    /// Create a new text renderer with the given font descriptions
    /// Fonts are specified as "Family:key=value" (e.g., "monospace:size=10")
    pub fn new(font_specs: &[String]) -> Self {
        let fonts: Vec<FontDescription> = font_specs
            .iter()
            .map(|spec| {
                // Convert polybar-style "Family:size=N" to Pango format "Family N"
                let pango_spec = Self::convert_font_spec(spec);
                let fd = FontDescription::from_string(&pango_spec);
                debug!("Loaded font: {} -> {}", spec, fd.to_string());
                fd
            })
            .collect();

        Self { fonts }
    }

    /// Convert polybar-style font spec to Pango format
    fn convert_font_spec(spec: &str) -> String {
        // Input: "JetBrains Mono:size=10" or "Font Awesome 6 Free:size=10"
        // Output: "JetBrains Mono 10" or "Font Awesome 6 Free 10"

        if let Some((family, params)) = spec.split_once(':') {
            let mut size: Option<i32> = None;
            let mut weight = None;
            let mut style = None;

            for param in params.split(':') {
                if let Some((key, value)) = param.split_once('=') {
                    match key.trim() {
                        "size" | "pixelsize" => size = value.trim().parse().ok(),
                        "weight" => weight = Some(value.trim().to_string()),
                        "style" => style = Some(value.trim().to_string()),
                        _ => {}
                    }
                }
            }

            let mut result = family.to_string();
            if let Some(w) = weight {
                result.push(' ');
                result.push_str(&w);
            }
            if let Some(s) = style {
                result.push(' ');
                result.push_str(&s);
            }
            if let Some(s) = size {
                result.push(' ');
                result.push_str(&s.to_string());
            }
            result
        } else {
            spec.to_string()
        }
    }

    /// Get the primary font description
    pub fn primary_font(&self) -> Option<&FontDescription> {
        self.fonts.first()
    }

    /// Create a Pango layout for the given Cairo context
    pub fn create_layout(&self, cr: &Context) -> Layout {
        let layout = pangocairo::create_layout(cr);

        // Set primary font
        if let Some(font) = self.primary_font() {
            layout.set_font_description(Some(font));
        }

        layout
    }

    /// Create a Pango layout with a specific font size override
    pub fn create_layout_with_size(&self, cr: &Context, font_size: f64) -> Layout {
        let layout = pangocairo::create_layout(cr);

        // Clone primary font and override size
        if let Some(font) = self.primary_font() {
            let mut sized_font = font.clone();
            sized_font.set_size((font_size * pango::SCALE as f64) as i32);
            layout.set_font_description(Some(&sized_font));
        }

        layout
    }

    /// Render text at the given position
    pub fn render(
        &self,
        cr: &Context,
        text: &str,
        x: f64,
        y: f64,
        color: &Color,
    ) -> Result<(f64, f64)> {
        let layout = self.create_layout(cr);
        layout.set_text(text);

        color.apply(cr);
        cr.move_to(x, y);
        pangocairo::show_layout(cr, &layout);

        let (width, height) = layout.pixel_size();
        Ok((width as f64, height as f64))
    }

    /// Render text with a maximum width, ellipsizing if needed
    pub fn render_ellipsized(
        &self,
        cr: &Context,
        text: &str,
        x: f64,
        y: f64,
        max_width: f64,
        color: &Color,
    ) -> Result<(f64, f64)> {
        let layout = self.create_layout(cr);
        layout.set_text(text);
        layout.set_width((max_width * pango::SCALE as f64) as i32);
        layout.set_ellipsize(EllipsizeMode::End);

        color.apply(cr);
        cr.move_to(x, y);
        pangocairo::show_layout(cr, &layout);

        let (width, height) = layout.pixel_size();
        Ok((width as f64, height as f64))
    }

    /// Measure text dimensions without rendering
    pub fn measure(&self, cr: &Context, text: &str) -> (f64, f64) {
        let layout = self.create_layout(cr);
        layout.set_text(text);
        let (width, height) = layout.pixel_size();
        (width as f64, height as f64)
    }

    /// Measure text dimensions with optional font size override
    pub fn measure_with_size(&self, cr: &Context, text: &str, font_size: Option<f64>) -> (f64, f64) {
        let layout = match font_size {
            Some(size) => self.create_layout_with_size(cr, size),
            None => self.create_layout(cr),
        };
        layout.set_text(text);
        let (width, height) = layout.pixel_size();
        (width as f64, height as f64)
    }

    /// Render text with ellipsis and optional font size override
    pub fn render_ellipsized_with_size(
        &self,
        cr: &Context,
        text: &str,
        x: f64,
        y: f64,
        max_width: f64,
        color: &Color,
        font_size: Option<f64>,
    ) -> Result<(f64, f64)> {
        let layout = match font_size {
            Some(size) => self.create_layout_with_size(cr, size),
            None => self.create_layout(cr),
        };
        layout.set_text(text);
        layout.set_width((max_width * pango::SCALE as f64) as i32);
        layout.set_ellipsize(EllipsizeMode::End);

        color.apply(cr);
        cr.move_to(x, y);
        pangocairo::show_layout(cr, &layout);

        let (width, height) = layout.pixel_size();
        Ok((width as f64, height as f64))
    }

    /// Measure text with a maximum width
    pub fn measure_ellipsized(&self, cr: &Context, text: &str, max_width: f64) -> (f64, f64) {
        let layout = self.create_layout(cr);
        layout.set_text(text);
        layout.set_width((max_width * pango::SCALE as f64) as i32);
        layout.set_ellipsize(EllipsizeMode::End);
        let (width, height) = layout.pixel_size();
        (width as f64, height as f64)
    }

    /// Get the line height for the primary font
    pub fn line_height(&self, cr: &Context) -> f64 {
        let layout = self.create_layout(cr);
        layout.set_text("Ay"); // Use chars with ascenders and descenders
        let (_, height) = layout.pixel_size();
        height as f64
    }
}

impl Default for TextRenderer {
    fn default() -> Self {
        Self::new(&["monospace 10".to_string()])
    }
}

/// Text alignment within a block
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

impl TextAlign {
    pub fn to_pango(self) -> PangoAlignment {
        match self {
            Self::Left => PangoAlignment::Left,
            Self::Center => PangoAlignment::Center,
            Self::Right => PangoAlignment::Right,
        }
    }
}
