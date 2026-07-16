//! Stable Wavora design tokens used by product-level component recipes.

/// Spacing values in logical pixels.
pub mod space {
    pub const XXS: f32 = 4.0;
    pub const XS: f32 = 5.0;
    pub const SM: f32 = 6.0;
    pub const MD: f32 = 8.0;
    pub const LG: f32 = 10.0;
    pub const XL: f32 = 12.0;
    pub const XXL: f32 = 14.0;
}

/// Corner radii in logical pixels.
pub mod radius {
    pub const CONTROL: f32 = 12.0;
    pub const CARD: f32 = 14.0;
    pub const PANEL: f32 = 20.0;
}

/// Typography sizes in logical pixels.
pub mod type_scale {
    pub const MICRO: f32 = 8.0;
    pub const CAPTION: f32 = 8.5;
    pub const EYEBROW: f32 = 9.0;
    pub const BODY_SMALL: f32 = 10.5;
    pub const CARD_TITLE: f32 = 16.0;
}

/// Alpha values for neutral white surfaces in the dark theme.
pub mod opacity {
    pub const NOTE_SURFACE: u8 = 5;
    pub const SECTION_SURFACE: u8 = 8;
    pub const CARD_SURFACE: u8 = 10;
    pub const HOVER_SURFACE: u8 = 24;
}
