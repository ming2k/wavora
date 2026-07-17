use iris::{Align, Color, Frame, LayoutOpts, TabStyle, TabsOpts};

use crate::tokens::{opacity, radius, space, type_scale};

/// A Wavora inspector tab strip built from Optics' opt-in indicator style.
///
/// The labels are presentation data and the selected index is returned to the
/// caller, keeping application state outside the component.
#[derive(Debug, Clone, Copy)]
pub struct InspectorTabs<'a> {
    id: &'a str,
    labels: &'a [&'a str],
    selected: usize,
    width: f32,
}

impl<'a> InspectorTabs<'a> {
    #[must_use]
    pub const fn new(id: &'a str, labels: &'a [&'a str], selected: usize, width: f32) -> Self {
        Self {
            id,
            labels,
            selected,
            width,
        }
    }

    /// Draw the strip and return its normalized selected index.
    #[must_use]
    pub fn show(self, frame: &mut Frame) -> usize {
        let Some(last_index) = self.labels.len().checked_sub(1) else {
            return 0;
        };
        let selected = self.selected.min(last_index);
        let mut active = i32::try_from(selected).unwrap_or(i32::MAX);

        frame.size_next(self.width.max(1.0), 0.0);
        frame.tabs_ex(
            self.id,
            &mut active,
            &TabsOpts {
                style: TabStyle::Indicator,
                hover_color: Color::rgba(255, 255, 255, opacity::HOVER_SURFACE),
                indicator_thickness: 2.5,
                indicator_gap: 2.0,
                indicator_padding: space::LG,
                equal_width: true,
                ..TabsOpts::default()
            },
            |frame| {
                for label in self.labels {
                    let _ = frame.tab(label);
                }
            },
        );

        usize::try_from(active).unwrap_or_default().min(last_index)
    }
}

/// Draw a titled inspector surface and return the body result.
pub fn inspector_section<R>(
    frame: &mut Frame,
    title: &str,
    body: impl FnOnce(&mut Frame) -> R,
) -> R {
    frame.column_ex(
        &LayoutOpts {
            gap: space::MD,
            pad: space::XL,
            cross: Align::Stretch,
            bg: Color::rgba(255, 255, 255, opacity::SECTION_SURFACE),
            radius: radius::CARD,
            ..LayoutOpts::default()
        },
        |frame| {
            frame.label_sized(title, type_scale::CAPTION);
            frame.separator();
            body(frame)
        },
    )
}

/// Add a quiet subgroup label inside an inspector section.
pub fn inspector_group(frame: &mut Frame, title: &str) {
    frame.spacer(space::XXS);
    frame.label_sized(title, type_scale::MICRO);
}

/// Draw a labelled inspector slider with a consistent numeric readout.
pub fn inspector_slider(
    frame: &mut Frame,
    label: &str,
    id: &str,
    value: &mut f32,
    min: f32,
    max: f32,
) -> bool {
    let readout = format!("{value:.2}");
    frame.row_ex(
        &LayoutOpts {
            height: 18.0,
            gap: space::SM,
            cross: Align::Center,
            ..LayoutOpts::default()
        },
        |frame| {
            frame.label_compact_sized(label, type_scale::CAPTION);
            frame.flex(1.0);
            frame.spacer(0.0);
            frame.label_compact_sized(&readout, type_scale::MICRO);
        },
    );
    frame.slider(id, value, min, max)
}

/// Draw low-emphasis explanatory copy below inspector controls.
pub fn inspector_note(frame: &mut Frame, copy: &str, available_width: f32) {
    frame.column_ex(
        &LayoutOpts {
            pad: space::LG,
            cross: Align::Stretch,
            bg: Color::rgba(255, 255, 255, opacity::NOTE_SURFACE),
            radius: radius::CONTROL,
            ..LayoutOpts::default()
        },
        |frame| {
            frame.label_wrapped_sized(
                copy,
                type_scale::CAPTION,
                (available_width - 48.0).max(120.0),
            );
        },
    );
}

#[cfg(test)]
mod tests {
    use iris::{Input, Ui};

    use super::*;

    #[test]
    fn inspector_tabs_normalize_an_out_of_range_selection() {
        let mut ui = Ui::headless().expect("headless Optics UI");
        let input = Input::new((320.0, 120.0), 1.0 / 60.0);
        let labels = ["Subject", "Ambient"];

        let selected = ui.frame(&input, |frame| {
            InspectorTabs::new("test-tabs", &labels, 8, 300.0).show(frame)
        });

        assert_eq!(selected, 1);
    }
}
