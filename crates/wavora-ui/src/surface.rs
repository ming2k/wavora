use iris::{Color, Frame, LayoutOpts};

use crate::tokens::{opacity, radius, space, type_scale};

/// A compact informational card used in Wavora overview layouts.
#[derive(Debug, Clone, Copy)]
pub struct InsightCard<'a> {
    eyebrow: &'a str,
    title: &'a str,
    subtitle: &'a str,
    height: f32,
}

impl<'a> InsightCard<'a> {
    #[must_use]
    pub const fn new(eyebrow: &'a str, title: &'a str, subtitle: &'a str) -> Self {
        Self {
            eyebrow,
            title,
            subtitle,
            height: 104.0,
        }
    }

    #[must_use]
    pub const fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    pub fn show(self, frame: &mut Frame) {
        frame.flex(1.0);
        frame.column_ex(
            &LayoutOpts {
                flex: 1.0,
                height: self.height.max(1.0),
                gap: space::XS,
                pad: space::XXL,
                bg: Color::rgba(255, 255, 255, opacity::CARD_SURFACE),
                radius: radius::CARD + 2.0,
                ..LayoutOpts::default()
            },
            |frame| {
                frame.label_sized(self.eyebrow, type_scale::EYEBROW);
                frame.label_sized(self.title, type_scale::CARD_TITLE);
                frame.label_sized(self.subtitle, type_scale::BODY_SMALL);
            },
        );
    }
}
