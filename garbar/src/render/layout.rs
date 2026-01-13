use cairo::Context;

use super::{Background, Color, TextRenderer};

/// Alignment of blocks within the bar
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
}

/// Padding specification
#[derive(Debug, Clone, Copy, Default)]
pub struct Padding {
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
}

impl Padding {
    pub const fn new(left: f64, right: f64, top: f64, bottom: f64) -> Self {
        Self { left, right, top, bottom }
    }

    pub const fn uniform(value: f64) -> Self {
        Self { left: value, right: value, top: value, bottom: value }
    }

    pub const fn horizontal(h: f64) -> Self {
        Self { left: h, right: h, top: 0.0, bottom: 0.0 }
    }

    pub const fn vertical(v: f64) -> Self {
        Self { left: 0.0, right: 0.0, top: v, bottom: v }
    }

    pub fn horizontal_total(&self) -> f64 {
        self.left + self.right
    }

    pub fn vertical_total(&self) -> f64 {
        self.top + self.bottom
    }
}

/// Margin specification (same structure as Padding)
pub type Margin = Padding;

/// Underline/overline style
#[derive(Debug, Clone)]
pub struct LineDecoration {
    pub width: f64,
    pub color: Color,
}

impl LineDecoration {
    pub fn new(width: f64, color: Color) -> Self {
        Self { width, color }
    }
}

/// Visual style for a block
#[derive(Debug, Clone, Default)]
pub struct BlockStyle {
    pub background: Background,
    pub foreground: Color,
    pub padding: Padding,
    pub margin: Margin,
    pub border_radius: f64,
    pub underline: Option<LineDecoration>,
    pub overline: Option<LineDecoration>,
}

impl BlockStyle {
    pub fn new() -> Self {
        Self {
            foreground: Color::white(),
            ..Default::default()
        }
    }

    pub fn with_foreground(mut self, color: Color) -> Self {
        self.foreground = color;
        self
    }

    pub fn with_background(mut self, bg: impl Into<Background>) -> Self {
        self.background = bg.into();
        self
    }

    pub fn with_padding(mut self, padding: Padding) -> Self {
        self.padding = padding;
        self
    }

    pub fn with_underline(mut self, width: f64, color: Color) -> Self {
        self.underline = Some(LineDecoration::new(width, color));
        self
    }

    pub fn with_overline(mut self, width: f64, color: Color) -> Self {
        self.overline = Some(LineDecoration::new(width, color));
        self
    }
}

/// A renderable block in the bar
#[derive(Debug, Clone)]
pub struct Block {
    pub text: String,
    pub style: BlockStyle,
    pub min_width: Option<f64>,
    pub max_width: Option<f64>,
}

impl Block {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: BlockStyle::new(),
            min_width: None,
            max_width: None,
        }
    }

    pub fn with_style(mut self, style: BlockStyle) -> Self {
        self.style = style;
        self
    }

    pub fn with_min_width(mut self, width: f64) -> Self {
        self.min_width = Some(width);
        self
    }

    pub fn with_max_width(mut self, width: f64) -> Self {
        self.max_width = Some(width);
        self
    }
}

/// Positioned block with computed layout
#[derive(Debug)]
pub struct PositionedBlock {
    pub block: Block,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl PositionedBlock {
    /// Render this block
    pub fn render(&self, cr: &Context, text_renderer: &TextRenderer) {
        let style = &self.block.style;
        let padding = &style.padding;

        // Draw background
        cr.save().unwrap();
        if style.border_radius > 0.0 {
            self.rounded_rect(cr, self.x, self.y, self.width, self.height, style.border_radius);
            cr.clip();
        }
        style.background.apply(cr, self.x, self.y, self.width, self.height);
        cr.rectangle(self.x, self.y, self.width, self.height);
        cr.fill().unwrap();
        cr.restore().unwrap();

        // Draw overline
        if let Some(overline) = &style.overline {
            overline.color.apply(cr);
            cr.set_line_width(overline.width);
            cr.move_to(self.x, self.y + overline.width / 2.0);
            cr.line_to(self.x + self.width, self.y + overline.width / 2.0);
            cr.stroke().unwrap();
        }

        // Draw underline
        if let Some(underline) = &style.underline {
            underline.color.apply(cr);
            cr.set_line_width(underline.width);
            cr.move_to(self.x, self.y + self.height - underline.width / 2.0);
            cr.line_to(self.x + self.width, self.y + self.height - underline.width / 2.0);
            cr.stroke().unwrap();
        }

        // Draw text
        let text_x = self.x + padding.left;
        let text_y = self.y + padding.top;
        let max_text_width = self.width - padding.horizontal_total();

        let _ = text_renderer.render_ellipsized(
            cr,
            &self.block.text,
            text_x,
            text_y,
            max_text_width,
            &style.foreground,
        );
    }

    fn rounded_rect(&self, cr: &Context, x: f64, y: f64, width: f64, height: f64, radius: f64) {
        let r = radius.min(width / 2.0).min(height / 2.0);
        cr.new_path();
        cr.arc(x + width - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
        cr.arc(x + width - r, y + height - r, r, 0.0, std::f64::consts::FRAC_PI_2);
        cr.arc(x + r, y + height - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
        cr.arc(x + r, y + r, r, std::f64::consts::PI, 3.0 * std::f64::consts::FRAC_PI_2);
        cr.close_path();
    }
}

/// Layout engine for arranging blocks
pub struct Layout {
    pub left: Vec<Block>,
    pub center: Vec<Block>,
    pub right: Vec<Block>,
}

impl Layout {
    pub fn new() -> Self {
        Self {
            left: Vec::new(),
            center: Vec::new(),
            right: Vec::new(),
        }
    }

    /// Compute positioned blocks for rendering
    pub fn compute(
        &self,
        cr: &Context,
        text_renderer: &TextRenderer,
        bar_width: f64,
        bar_height: f64,
        bar_padding: &Padding,
    ) -> Vec<PositionedBlock> {
        let mut result = Vec::new();

        let content_width = bar_width - bar_padding.horizontal_total();
        let content_start_x = bar_padding.left;

        // Measure all blocks
        let measure_blocks = |blocks: &[Block]| -> Vec<(f64, f64)> {
            blocks
                .iter()
                .map(|b| {
                    let (text_w, text_h) = text_renderer.measure(cr, &b.text);
                    let w = text_w + b.style.padding.horizontal_total() + b.style.margin.horizontal_total();
                    let w = b.min_width.map(|m| w.max(m)).unwrap_or(w);
                    let w = b.max_width.map(|m| w.min(m)).unwrap_or(w);
                    (w, text_h + b.style.padding.vertical_total())
                })
                .collect()
        };

        let left_sizes = measure_blocks(&self.left);
        let center_sizes = measure_blocks(&self.center);
        let right_sizes = measure_blocks(&self.right);

        let _left_total: f64 = left_sizes.iter().map(|(w, _)| w).sum();
        let center_total: f64 = center_sizes.iter().map(|(w, _)| w).sum();
        let _right_total: f64 = right_sizes.iter().map(|(w, _)| w).sum();

        // Position left blocks
        let mut x = content_start_x;
        for (block, (width, _)) in self.left.iter().zip(&left_sizes) {
            let margin = &block.style.margin;
            x += margin.left;
            let block_width = width - margin.horizontal_total();
            // Vertically centered within bar (simplified for now)

            result.push(PositionedBlock {
                block: block.clone(),
                x,
                y: bar_padding.top,
                width: block_width,
                height: bar_height - bar_padding.vertical_total(),
            });

            x += block_width + margin.right;
        }

        // Position center blocks
        let center_start = content_start_x + (content_width - center_total) / 2.0;
        let mut x = center_start;
        for (block, (width, _)) in self.center.iter().zip(&center_sizes) {
            let margin = &block.style.margin;
            x += margin.left;
            let block_width = width - margin.horizontal_total();

            result.push(PositionedBlock {
                block: block.clone(),
                x,
                y: bar_padding.top,
                width: block_width,
                height: bar_height - bar_padding.vertical_total(),
            });

            x += block_width + margin.right;
        }

        // Position right blocks
        let mut x = content_start_x + content_width;
        for (block, (width, _)) in self.right.iter().zip(&right_sizes).rev() {
            let margin = &block.style.margin;
            x -= margin.right;
            let block_width = width - margin.horizontal_total();
            x -= block_width;

            result.push(PositionedBlock {
                block: block.clone(),
                x,
                y: bar_padding.top,
                width: block_width,
                height: bar_height - bar_padding.vertical_total(),
            });

            x -= margin.left;
        }

        result
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self::new()
    }
}
