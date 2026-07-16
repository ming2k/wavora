use iris::{Frame, Icon};

/// Styling inputs for one Wavora player control.
#[derive(Debug, Clone, Copy)]
pub struct PlayerControlButton<'a> {
    icon: Icon,
    badge: &'a str,
    icon_size: f32,
    active: bool,
}

impl<'a> PlayerControlButton<'a> {
    #[must_use]
    pub const fn new(icon: Icon) -> Self {
        Self {
            icon,
            badge: "",
            icon_size: 26.0,
            active: false,
        }
    }

    #[must_use]
    pub const fn badge(mut self, badge: &'a str) -> Self {
        self.badge = badge;
        self
    }

    #[must_use]
    pub const fn icon_size(mut self, icon_size: f32) -> Self {
        self.icon_size = icon_size;
        self
    }

    #[must_use]
    pub const fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }
}

/// Draw one player control and return whether it was activated.
pub fn player_control_button(frame: &mut Frame, button: PlayerControlButton<'_>) -> bool {
    frame.icon_button_badged(
        button.icon,
        button.badge,
        button.icon_size.max(1.0),
        button.active,
    )
}
