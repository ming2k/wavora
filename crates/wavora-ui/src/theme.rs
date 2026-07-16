use iris::{Color, Theme};

use crate::tokens::radius;

/// Build the Wavora dark theme around a caller-provided product accent.
///
/// Palette selection remains an application concern; this function owns the
/// stable relationship between that accent and Wavora's semantic UI tokens.
#[must_use]
pub fn theme(accent: [u8; 3]) -> Theme {
    Theme::dark()
        .with_bg(Color::rgba(3, 5, 8, 255))
        .with_fg(Color::rgba(232, 236, 239, 255))
        .with_accent(Color::rgba(accent[0], accent[1], accent[2], 255))
        .with_border(Color::rgba(255, 255, 255, 22))
        .with_hover(Color::rgba(255, 255, 255, 16))
        .with_active(Color::rgba(accent[0], accent[1], accent[2], 38))
        .with_disabled(Color::rgba(138, 144, 153, 150))
        .with_error(Color::rgba(255, 96, 116, 255))
        .with_font_size(14.0)
        .with_corner_radius(radius::CONTROL)
        .with_border_width(1.0)
        .with_active_indicator_width(0.0)
        .with_scrollbar_width(8.0)
        .with_scrollbar_radius(4.0)
        .with_scrollbar_min_thumb_h(38.0)
        .with_scrollbar_track_color(Color::rgba(255, 255, 255, 10))
        .with_scrollbar_thumb_color(Color::rgba(255, 255, 255, 54))
        .with_scrollbar_thumb_hover_color(Color::rgba(255, 255, 255, 92))
        .with_scrollbar_thumb_active_color(Color::rgba(accent[0], accent[1], accent[2], 190))
        .with_slider_track_color(Color::rgba(255, 255, 255, 42))
        .with_slider_fill_color(Color::rgba(accent[0], accent[1], accent[2], 255))
        .with_slider_knob_color(Color::rgba(250, 251, 253, 255))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accent_drives_semantic_active_tokens() {
        let accent = [112, 246, 218];
        let theme = theme(accent);

        assert_eq!(theme.accent(), Color::rgba(112, 246, 218, 255));
        assert_eq!(theme.active(), Color::rgba(112, 246, 218, 38));
        assert!(theme.is_dark());
    }
}
