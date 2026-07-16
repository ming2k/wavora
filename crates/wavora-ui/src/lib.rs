//! Wavora's product-level UI composition layer.
//!
//! Optics owns generic layout, interaction, accessibility, and animation
//! primitives. This crate combines those primitives with Wavora's design
//! tokens. Components accept presentation data and return interaction results;
//! they intentionally do not depend on application state, media services,
//! localization tables, or the visual renderer.

mod inspector;
mod player;
mod surface;
mod theme;

pub mod tokens;

pub use inspector::{
    InspectorTabs, inspector_group, inspector_note, inspector_section, inspector_slider,
};
pub use player::{PlayerControlButton, player_control_button};
pub use surface::InsightCard;
pub use theme::theme;
