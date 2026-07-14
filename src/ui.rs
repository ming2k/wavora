use crate::app::{App, View};
use iris::{Align, Color, Frame, Icon, LayoutOpts, TableColumn, TableOpts, Theme};
use wavora_core::format_duration;
use wavora_i18n::{Key, Language, LanguagePreference, text, visual_preset_text};
use wavora_visuals::PRESETS;

const ROOT_PAD: f32 = 18.0;
const GAP: f32 = 14.0;
const TOP_BAR_HEIGHT: f32 = 54.0;
const COMPACT_NAV_HEIGHT: f32 = 46.0;
const STATUS_HEIGHT: f32 = 40.0;
const PLAYER_HEIGHT: f32 = 100.0;
const VISUAL_RAIL_WIDTH: f32 = 236.0;
const VISUAL_STAGE_GAP: f32 = 12.0;

#[must_use]
pub fn theme(preset: usize) -> Theme {
    let accent = PRESETS[preset % PRESETS.len()].accent;
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
        .with_corner_radius(12.0)
        .with_border_width(1.0)
        .with_active_indicator_width(3.0)
        .with_scrollbar_width(8.0)
        .with_scrollbar_radius(4.0)
        .with_scrollbar_min_thumb_h(38.0)
        .with_scrollbar_track_color(Color::rgba(255, 255, 255, 10))
        .with_scrollbar_thumb_color(Color::rgba(255, 255, 255, 54))
        .with_scrollbar_thumb_hover_color(Color::rgba(255, 255, 255, 92))
        .with_scrollbar_thumb_active_color(Color::rgba(accent[0], accent[1], accent[2], 190))
}

pub fn build(app: &mut App, frame: &mut Frame, width: f32, height: f32) {
    frame.set_theme(theme(app.preset));
    let show_sidebar = width >= 760.0;
    let show_queue = width >= 1_300.0;
    let status_visible = app.toast_message().is_some();
    let language = app.language();
    let sidebar_width = if show_sidebar { 190.0 } else { 0.0 };
    let queue_width = if show_queue { 272.0 } else { 0.0 };
    let panel_width = (width
        - ROOT_PAD * 2.0
        - sidebar_width
        - queue_width
        - if show_sidebar { GAP } else { 0.0 }
        - if show_queue { GAP } else { 0.0 })
    .max(320.0);
    let root_gap_count =
        2.0 + if show_sidebar { 0.0 } else { 1.0 } + if status_visible { 1.0 } else { 0.0 };
    let chrome_height = TOP_BAR_HEIGHT
        + if show_sidebar {
            0.0
        } else {
            COMPACT_NAV_HEIGHT
        }
        + if status_visible { STATUS_HEIGHT } else { 0.0 }
        + PLAYER_HEIGHT
        + GAP * root_gap_count
        + ROOT_PAD * 2.0;
    let content_height = (height - chrome_height).max(220.0);
    let content_y = ROOT_PAD
        + TOP_BAR_HEIGHT
        + GAP
        + if show_sidebar {
            0.0
        } else {
            COMPACT_NAV_HEIGHT + GAP
        }
        + if status_visible {
            STATUS_HEIGHT + GAP
        } else {
            0.0
        };
    let content_x = ROOT_PAD
        + if show_sidebar {
            sidebar_width + GAP
        } else {
            0.0
        };
    if app.view == View::Visuals {
        let inner_width = (panel_width - 44.0).max(1.0);
        let inner_height = (content_height - 44.0).max(1.0);
        let side_controls = inner_width >= 620.0;
        let stage_width = if side_controls {
            (inner_width - VISUAL_RAIL_WIDTH - VISUAL_STAGE_GAP).max(1.0)
        } else {
            inner_width
        };
        let stage_height = if side_controls {
            inner_height
        } else {
            stacked_visual_stage_height(inner_height)
        };
        app.set_visual_viewport(Some((
            content_x + 22.0,
            content_y + 22.0,
            stage_width,
            stage_height,
        )));
    } else {
        app.set_visual_viewport(None);
    }

    frame.column_ex(
        &LayoutOpts {
            flex: 1.0,
            gap: GAP,
            pad: ROOT_PAD,
            cross: Align::Stretch,
            ..LayoutOpts::default()
        },
        |frame| {
            top_bar(app, frame, width, language);
            if !show_sidebar {
                compact_navigation(app, frame, language);
            }
            if status_visible {
                status_banner(app, frame, width);
            }
            frame.flex(1.0);
            frame.row_ex(
                &LayoutOpts {
                    flex: 1.0,
                    gap: GAP,
                    cross: Align::Stretch,
                    ..LayoutOpts::default()
                },
                |frame| {
                    if show_sidebar {
                        sidebar(app, frame, language);
                    }
                    frame.flex(1.0);
                    main_content(app, frame, panel_width, content_height, language);
                    if show_queue {
                        queue(app, frame, queue_width, content_height, language);
                    }
                },
            );
            player_bar(app, frame, width, language);
        },
    );
}

fn top_bar(app: &mut App, frame: &mut Frame, width: f32, language: Language) {
    frame.size_next(0.0, TOP_BAR_HEIGHT);
    frame.row_ex(
        &LayoutOpts {
            height: TOP_BAR_HEIGHT,
            gap: 10.0,
            cross: Align::Center,
            ..LayoutOpts::default()
        },
        |frame| {
            frame.label_compact_sized("WAVORA", 19.0);
            if width >= 860.0 {
                frame
                    .label_compact_sized(&format!("/  {}", text(language, Key::AppSubtitle)), 10.5);
            }
            frame.flex(1.0);
            frame.spacer(0.0);
            frame.size_next(42.0, 38.0);
            if frame.icon_button(Icon::Plus) {
                app.pick_music_file();
            }
            frame.size_next(42.0, 38.0);
            if frame.icon_button(Icon::Database) {
                app.pick_music_folder();
            }
            frame.size_next(42.0, 38.0);
            if frame.icon_button_active(Icon::Settings, app.view == View::Settings) {
                app.view = View::Settings;
            }
        },
    );
}

fn compact_navigation(app: &mut App, frame: &mut Frame, language: Language) {
    frame.size_next(0.0, COMPACT_NAV_HEIGHT);
    frame.row_ex(
        &LayoutOpts {
            height: COMPACT_NAV_HEIGHT,
            gap: 6.0,
            cross: Align::Center,
            ..LayoutOpts::default()
        },
        |frame| {
            compact_nav_item(app, frame, Icon::Home, text(language, Key::Now), View::Home);
            compact_nav_item(
                app,
                frame,
                Icon::Database,
                text(language, Key::Library),
                View::Library,
            );
            compact_nav_item(
                app,
                frame,
                Icon::Star,
                text(language, Key::Favorites),
                View::Favorites,
            );
            compact_nav_item(
                app,
                frame,
                Icon::Activity,
                text(language, Key::VisualStage),
                View::Visuals,
            );
        },
    );
}

fn compact_nav_item(app: &mut App, frame: &mut Frame, icon: Icon, label: &str, view: View) {
    frame.flex(1.0);
    frame.size_next(0.0, 38.0);
    if frame.selectable_icon(icon, label, app.view == view) {
        app.view = view;
    }
}

fn status_banner(app: &App, frame: &mut Frame, width: f32) {
    let Some(message) = app.toast_message() else {
        return;
    };
    let max_chars = if width >= 1_200.0 {
        120
    } else if width >= 900.0 {
        84
    } else if width >= 700.0 {
        58
    } else if width >= 500.0 {
        38
    } else {
        24
    };
    let message = ellipsize(message, max_chars);
    let background = if app.status_is_error() {
        Color::rgba(255, 96, 116, 22)
    } else {
        let accent = PRESETS[app.preset % PRESETS.len()].accent;
        Color::rgba(accent[0], accent[1], accent[2], 20)
    };
    frame.size_next(0.0, STATUS_HEIGHT);
    frame.row_ex(
        &LayoutOpts {
            height: STATUS_HEIGHT,
            pad: 10.0,
            cross: Align::Center,
            bg: background,
            radius: 12.0,
            ..LayoutOpts::default()
        },
        |frame| frame.label_compact_sized(&message, 11.5),
    );
}

fn sidebar(app: &mut App, frame: &mut Frame, language: Language) {
    frame.size_next(190.0, 0.0);
    frame.column_ex(
        &LayoutOpts {
            width: 190.0,
            flex: 1.0,
            gap: 7.0,
            pad: 14.0,
            cross: Align::Stretch,
            bg: Color::rgba(9, 12, 17, 208),
            radius: 22.0,
            ..LayoutOpts::default()
        },
        |frame| {
            frame.label_sized(text(language, Key::YourSpace), 9.5);
            nav_item(app, frame, Icon::Home, text(language, Key::Now), View::Home);
            nav_item(
                app,
                frame,
                Icon::Database,
                text(language, Key::Library),
                View::Library,
            );
            nav_item(
                app,
                frame,
                Icon::Star,
                text(language, Key::Favorites),
                View::Favorites,
            );
            nav_item(
                app,
                frame,
                Icon::Activity,
                text(language, Key::VisualStage),
                View::Visuals,
            );
            frame.spacer(10.0);
            frame.separator();
            frame.spacer(8.0);
            frame.label_sized(text(language, Key::Collection), 9.5);
            frame.label(&format!(
                "{} {}",
                app.tracks.len(),
                text(language, Key::LocalTracks)
            ));
            frame.label(&format!(
                "{} {}",
                app.favorite_count(),
                text(language, Key::FavoriteTracks)
            ));
            frame.flex(1.0);
            frame.spacer(0.0);
            frame.label_sized(
                if app.scanning {
                    text(language, Key::Scanning)
                } else {
                    "Rodio · Symphonia · Optics"
                },
                10.0,
            );
        },
    );
}

fn nav_item(app: &mut App, frame: &mut Frame, icon: Icon, label: &str, view: View) {
    frame.size_next(0.0, 40.0);
    if frame.selectable_icon(icon, label, app.view == view) {
        app.view = view;
    }
}

fn main_content(app: &mut App, frame: &mut Frame, width: f32, height: f32, language: Language) {
    let background = if app.view == View::Visuals {
        Color::rgba(6, 9, 14, 92)
    } else {
        Color::rgba(8, 11, 16, 194)
    };
    frame.column_ex(
        &LayoutOpts {
            flex: 1.0,
            gap: 12.0,
            pad: 22.0,
            cross: Align::Stretch,
            bg: background,
            radius: 24.0,
            ..LayoutOpts::default()
        },
        |frame| match app.view {
            View::Home => home(app, frame, width, language),
            View::Library => library(app, frame, false, width, height, language),
            View::Favorites => library(app, frame, true, width, height, language),
            View::Visuals => visuals(app, frame, width, height, language),
            View::Settings => settings(app, frame, width, language),
        },
    );
}

fn home(app: &mut App, frame: &mut Frame, width: f32, language: Language) {
    frame.label_sized(text(language, Key::ImmersiveListening), 10.0);
    if let Some(track) = app.current_track() {
        let title = ellipsize(&track.title, if width > 760.0 { 72 } else { 40 });
        let artist_album = format!("{}  ·  {}", track.artist, track.album);
        frame.spacer(18.0);
        frame.label_sized(&title, if width > 760.0 { 34.0 } else { 27.0 });
        frame.label_wrapped_sized(&artist_album, 14.0, (width - 60.0).max(260.0));
        frame.spacer(22.0);
        frame.label_wrapped_sized(
            text(language, Key::VisualDescription),
            13.0,
            (width - 60.0).max(260.0),
        );
    } else {
        frame.spacer(20.0);
        frame.label_sized(
            text(language, Key::SoundInMotion),
            if width > 760.0 { 36.0 } else { 28.0 },
        );
        frame.label_wrapped_sized(
            text(language, Key::EmptyHomeDescription),
            13.0,
            (width - 60.0).max(260.0),
        );
        frame.spacer(22.0);
        frame.size_next(220.0, 42.0);
        if frame.selectable(text(language, Key::AddMusicFolder), false) {
            app.pick_music_folder();
        }
    }
    frame.flex(1.0);
    frame.row_ex(
        &LayoutOpts {
            gap: 10.0,
            cross: Align::Stretch,
            ..LayoutOpts::default()
        },
        |frame| {
            insight_card(
                frame,
                text(language, Key::LibraryCard),
                &format!("{} {}", app.tracks.len(), text(language, Key::Tracks)),
                text(language, Key::LocalArchive),
            );
            if width >= 720.0 {
                insight_card(
                    frame,
                    text(language, Key::VisualCard),
                    visual_preset_text(language, app.preset).name,
                    visual_preset_text(language, app.preset).subtitle,
                );
                insight_card(
                    frame,
                    text(language, Key::EngineCard),
                    "SYMPHONIA",
                    text(language, Key::SystemDecode),
                );
            }
        },
    );
}

fn insight_card(frame: &mut Frame, eyebrow: &str, title: &str, subtitle: &str) {
    frame.flex(1.0);
    frame.column_ex(
        &LayoutOpts {
            flex: 1.0,
            height: 104.0,
            gap: 5.0,
            pad: 14.0,
            bg: Color::rgba(255, 255, 255, 10),
            radius: 16.0,
            ..LayoutOpts::default()
        },
        |frame| {
            frame.label_sized(eyebrow, 9.0);
            frame.label_sized(title, 16.0);
            frame.label_sized(subtitle, 10.5);
        },
    );
}

fn library(
    app: &mut App,
    frame: &mut Frame,
    favorites_only: bool,
    width: f32,
    height: f32,
    language: Language,
) {
    let visible = app.visible_track_indices(favorites_only);
    frame.row_ex(
        &LayoutOpts {
            height: 38.0,
            cross: Align::Center,
            ..LayoutOpts::default()
        },
        |frame| {
            frame.heading(
                if favorites_only {
                    text(language, Key::Favorites)
                } else {
                    text(language, Key::LocalLibrary)
                },
                1,
            );
            frame.flex(1.0);
            frame.spacer(0.0);
            frame.label_sized(
                &format!("{} {}", visible.len(), text(language, Key::Tracks)),
                10.0,
            );
        },
    );
    frame.size_next(0.0, 38.0);
    if frame.textfield(text(language, Key::Search), &mut app.search) {
        app.view = if favorites_only {
            View::Favorites
        } else {
            View::Library
        };
    }
    if visible.is_empty() {
        frame.flex(1.0);
        frame.label_wrapped(text(language, Key::EmptyLibrary), (width - 60.0).max(240.0));
        return;
    }

    let show_album = width >= 760.0;
    let mut columns = vec![
        TableColumn {
            title: text(language, Key::Title),
            width: 0.0,
            align: Align::Start,
        },
        TableColumn {
            title: text(language, Key::Artist),
            width: if show_album { 170.0 } else { 142.0 },
            align: Align::Start,
        },
    ];
    if show_album {
        columns.push(TableColumn {
            title: text(language, Key::Album),
            width: 160.0,
            align: Align::Start,
        });
    }
    columns.push(TableColumn {
        title: text(language, Key::Duration),
        width: 70.0,
        align: Align::End,
    });
    let table_width = (width - 44.0).max(280.0);
    let table_height = (height - 116.0).max(160.0);
    let fixed_width = columns
        .iter()
        .filter(|column| column.width > 0.0)
        .map(|column| column.width)
        .sum::<f32>();
    let title_cell_width = (table_width - fixed_width - 36.0).max(80.0);
    let title_chars = if title_cell_width >= 500.0 {
        60
    } else if title_cell_width >= 360.0 {
        44
    } else if title_cell_width >= 240.0 {
        30
    } else if title_cell_width >= 150.0 {
        18
    } else {
        10
    };
    frame.size_next(table_width, table_height);
    let tracks = &app.tracks;
    let result = frame.table(
        if favorites_only {
            "favorites-table"
        } else {
            "library-table"
        },
        &columns,
        visible.len(),
        TableOpts {
            row_height: 46.0,
            show_header: true,
            selectable: true,
            zebra: true,
        },
        |row, column| {
            let Some(track) = visible.get(row).and_then(|index| tracks.get(*index)) else {
                return String::new();
            };
            if show_album {
                match column {
                    0 => ellipsize(&track.title, title_chars),
                    1 => ellipsize(&track.artist, 21),
                    2 => ellipsize(&track.album, 20),
                    3 => format_duration(track.duration_ms),
                    _ => String::new(),
                }
            } else {
                match column {
                    0 => ellipsize(&track.title, title_chars),
                    1 => ellipsize(&track.artist, 17),
                    2 => format_duration(track.duration_ms),
                    _ => String::new(),
                }
            }
        },
    );
    if result.clicked
        && let Some(row) = result.selected
        && let Some(index) = visible.get(row).copied()
    {
        app.play_index(index);
    }
}

fn visuals(app: &mut App, frame: &mut Frame, width: f32, height: f32, language: Language) {
    let inner_width = (width - 44.0).max(1.0);
    let inner_height = (height - 44.0).max(1.0);
    if inner_width >= 620.0 {
        let stage_width = (inner_width - VISUAL_RAIL_WIDTH - VISUAL_STAGE_GAP).max(1.0);
        frame.row_ex(
            &LayoutOpts {
                flex: 1.0,
                gap: VISUAL_STAGE_GAP,
                cross: Align::Stretch,
                ..LayoutOpts::default()
            },
            |frame| {
                visual_stage(app, frame, stage_width, inner_height, language);
                visual_controls(app, frame, VISUAL_RAIL_WIDTH, language);
            },
        );
    } else {
        let stage_height = stacked_visual_stage_height(inner_height);
        frame.column_ex(
            &LayoutOpts {
                flex: 1.0,
                gap: VISUAL_STAGE_GAP,
                cross: Align::Stretch,
                ..LayoutOpts::default()
            },
            |frame| {
                visual_stage(app, frame, inner_width, stage_height, language);
                visual_controls(app, frame, inner_width, language);
            },
        );
    }
}

fn stacked_visual_stage_height(inner_height: f32) -> f32 {
    (inner_height * 0.46).clamp(210.0, 300.0).min(inner_height)
}

fn visual_stage(app: &App, frame: &mut Frame, width: f32, height: f32, language: Language) {
    let copy = visual_preset_text(language, app.preset);
    let features = app.live_audio_features();
    let is_live = app.playback_state.is_playing();
    frame.column_ex(
        &LayoutOpts {
            flex: 1.0,
            width,
            height,
            gap: 6.0,
            pad: 16.0,
            cross: Align::Stretch,
            bg: Color::rgba(3, 6, 11, 42),
            radius: 18.0,
        },
        |frame| {
            frame.row_ex(
                &LayoutOpts {
                    gap: 8.0,
                    cross: Align::Center,
                    ..LayoutOpts::default()
                },
                |frame| {
                    frame.label_sized(text(language, Key::VisualPreview), 8.5);
                    frame.flex(1.0);
                    frame.label_sized(
                        if is_live {
                            text(language, Key::Live)
                        } else {
                            text(language, Key::WaitingForAudio)
                        },
                        8.5,
                    );
                },
            );
            frame.label_sized(copy.name, if width >= 430.0 { 27.0 } else { 22.0 });
            frame.label_wrapped_sized(copy.subtitle, 11.0, (width - 32.0).max(120.0));
            frame.label_wrapped_sized(copy.response, 9.5, (width - 32.0).max(120.0));
            frame.flex(1.0);
            frame.spacer(0.0);
            if height >= 340.0 {
                visual_metrics(frame, features, language);
            } else {
                let pitch = if features.pitch_confidence > 0.2 {
                    format!("{:.0} Hz", features.pitch_hz)
                } else {
                    "—".to_owned()
                };
                frame.label_wrapped_sized(
                    &format!(
                        "{} {:.1} dBFS  ·  {} {pitch}  ·  {} {:.0}%",
                        text(language, Key::Loudness),
                        features.loudness_db,
                        text(language, Key::Pitch),
                        text(language, Key::Onset),
                        features.onset * 100.0
                    ),
                    9.0,
                    (width - 32.0).max(120.0),
                );
            }
        },
    );
}

fn visual_metrics(frame: &mut Frame, features: wavora_media::AudioFeatures, language: Language) {
    let pitch = if features.pitch_confidence > 0.2 {
        format!("{:.0} Hz", features.pitch_hz)
    } else {
        "—".to_owned()
    };
    let values = [
        format!("{:.1} dBFS", features.loudness_db),
        pitch,
        format!("{:.0} Hz", features.spectral_centroid_hz),
        format!("{:.0}%", features.onset * 100.0),
    ];
    let labels = [
        text(language, Key::Loudness),
        text(language, Key::Pitch),
        text(language, Key::Centroid),
        text(language, Key::Onset),
    ];
    frame.row_ex(
        &LayoutOpts {
            height: 58.0,
            gap: 6.0,
            cross: Align::Stretch,
            ..LayoutOpts::default()
        },
        |frame| {
            for (label, value) in labels.into_iter().zip(values) {
                frame.flex(1.0);
                frame.column_ex(
                    &LayoutOpts {
                        flex: 1.0,
                        height: 58.0,
                        gap: 2.0,
                        pad: 8.0,
                        bg: Color::rgba(255, 255, 255, 12),
                        radius: 11.0,
                        ..LayoutOpts::default()
                    },
                    |frame| {
                        frame.label_compact_sized(label, 8.0);
                        frame.label_compact_sized(&value, 10.0);
                    },
                );
            }
        },
    );
}

fn visual_controls(app: &mut App, frame: &mut Frame, width: f32, language: Language) {
    frame.column_ex(
        &LayoutOpts {
            flex: 1.0,
            width,
            gap: 7.0,
            pad: 14.0,
            cross: Align::Stretch,
            bg: Color::rgba(5, 8, 13, 224),
            radius: 18.0,
            ..LayoutOpts::default()
        },
        |frame| {
            frame.heading(text(language, Key::VisualControls), 2);
            frame.label_sized(
                if app.playback_state.is_playing() {
                    text(language, Key::Listening)
                } else {
                    text(language, Key::WaitingForAudio)
                },
                9.0,
            );
            frame.separator();
            frame.flex(1.0);
            frame.scroll("visual-control-scroll", |frame| {
                frame.label_sized(text(language, Key::VisualPresets), 8.5);
                for index in 0..PRESETS.len() {
                    let copy = visual_preset_text(language, index);
                    frame.size_next(0.0, 36.0);
                    if frame.selectable(copy.name, app.preset == index) {
                        app.set_preset(index);
                    }
                }
                frame.spacer(5.0);
                frame.separator();
                frame.spacer(5.0);
                let mut changed = false;
                frame.label_sized(text(language, Key::Intensity), 8.5);
                changed |= frame.slider(
                    "##visual-intensity",
                    &mut app.visual_tuning.intensity,
                    0.45,
                    1.75,
                );
                frame.label_sized(text(language, Key::Motion), 8.5);
                changed |=
                    frame.slider("##visual-motion", &mut app.visual_tuning.motion, 0.35, 1.65);
                frame.label_sized(text(language, Key::Depth), 8.5);
                changed |= frame.slider("##visual-depth", &mut app.visual_tuning.depth, 0.50, 1.50);
                frame.label_sized(text(language, Key::Glow), 8.5);
                changed |= frame.slider("##visual-glow", &mut app.visual_tuning.glow, 0.25, 1.50);
                if changed {
                    app.apply_visual_tuning();
                }
                frame.spacer(8.0);
                frame.label_wrapped_sized(
                    text(language, Key::VisualFootnote),
                    8.5,
                    (width - 28.0).max(120.0),
                );
            });
        },
    );
}

fn settings(app: &mut App, frame: &mut Frame, width: f32, language: Language) {
    frame.heading(text(language, Key::Settings), 1);
    frame.label_wrapped(
        text(language, Key::SettingsDescription),
        (width - 60.0).max(260.0),
    );
    frame.spacer(12.0);
    if frame.slider(
        text(language, Key::DefaultVolume),
        &mut app.volume,
        0.0,
        1.0,
    ) {
        app.apply_volume();
    }
    frame.spacer(8.0);
    let mut selected = match app.language_preference() {
        LanguagePreference::System => 0,
        LanguagePreference::English => 1,
        LanguagePreference::SimplifiedChinese => 2,
    };
    let choices = [
        text(language, Key::FollowSystem),
        text(language, Key::English),
        text(language, Key::SimplifiedChinese),
    ];
    if frame.dropdown(text(language, Key::Language), &mut selected, &choices) {
        let preference = match selected {
            1 => LanguagePreference::English,
            2 => LanguagePreference::SimplifiedChinese,
            _ => LanguagePreference::System,
        };
        app.set_language_preference(preference);
    }
    frame.spacer(12.0);
    frame.label_wrapped(
        &format!(
            "{}: {}",
            text(language, Key::ConfigFile),
            app.config_path().display()
        ),
        (width - 60.0).max(260.0),
    );
    frame.label(text(language, Key::MusicFolders));
    if app.library_roots().is_empty() {
        frame.label_sized(text(language, Key::NoMusicFolder), 11.0);
    } else {
        frame.size_next(0.0, 110.0);
        frame.scroll("settings-roots", |frame| {
            for root in app.library_roots() {
                frame.label_wrapped_sized(&root.display().to_string(), 10.5, width - 86.0);
            }
        });
    }
    frame.label_sized(text(language, Key::SupportedFormats), 10.5);
    frame.flex(1.0);
    frame.label_sized(
        &format!("Wavora {} · Rust + Optics", env!("CARGO_PKG_VERSION")),
        10.0,
    );
}

fn queue(app: &mut App, frame: &mut Frame, width: f32, height: f32, language: Language) {
    frame.size_next(width, 0.0);
    frame.column_ex(
        &LayoutOpts {
            width,
            flex: 1.0,
            gap: 8.0,
            pad: 16.0,
            cross: Align::Stretch,
            bg: Color::rgba(9, 12, 17, 208),
            radius: 22.0,
            ..LayoutOpts::default()
        },
        |frame| {
            frame.row(|frame| {
                frame.heading(text(language, Key::UpNext), 2);
                frame.flex(1.0);
                frame.label_sized(text(language, Key::Queue), 9.0);
            });
            frame.separator();
            frame.size_next(0.0, (height - 70.0).max(150.0));
            frame.scroll("queue-scroll", |frame| {
                if app.tracks.is_empty() {
                    frame.label_sized(text(language, Key::EmptyQueue), 11.0);
                }
                for index in app.queue_indices() {
                    let track = &app.tracks[index];
                    let label = ellipsize(&format!("{}  ·  {}", track.title, track.artist), 34);
                    frame.size_next(0.0, 48.0);
                    if frame.selectable(&label, app.current_index == Some(index)) {
                        app.play_index(index);
                    }
                }
            });
        },
    );
}

fn player_bar(app: &mut App, frame: &mut Frame, width: f32, language: Language) {
    frame.size_next(0.0, PLAYER_HEIGHT);
    frame.row_ex(
        &LayoutOpts {
            height: PLAYER_HEIGHT,
            gap: 12.0,
            pad: 14.0,
            cross: Align::Center,
            bg: Color::rgba(7, 9, 13, 232),
            radius: 24.0,
            ..LayoutOpts::default()
        },
        |frame| {
            if width >= 720.0 {
                let current = app.current_track().map(|track| {
                    (
                        ellipsize(&track.title, 30),
                        ellipsize(&track.artist, 24),
                        track.codec.clone(),
                        track.favorite,
                    )
                });
                frame.size_next(if width >= 1_100.0 { 256.0 } else { 210.0 }, 66.0);
                frame.column_ex(
                    &LayoutOpts {
                        width: if width >= 1_100.0 { 256.0 } else { 210.0 },
                        height: 66.0,
                        gap: 3.0,
                        pad: 8.0,
                        bg: Color::rgba(255, 255, 255, 9),
                        radius: 15.0,
                        ..LayoutOpts::default()
                    },
                    |frame| {
                        if let Some((title, artist, codec, favorite)) = current.as_ref() {
                            frame.row_ex(
                                &LayoutOpts {
                                    gap: 4.0,
                                    cross: Align::Center,
                                    ..LayoutOpts::default()
                                },
                                |frame| {
                                    frame.flex(1.0);
                                    frame.label_compact_sized(title, 13.5);
                                    frame.size_next(28.0, 24.0);
                                    if frame.icon_button_active(Icon::Star, *favorite) {
                                        app.toggle_current_favorite();
                                    }
                                },
                            );
                            frame.label_compact_sized(&format!("{artist}  ·  {codec}"), 10.0);
                        } else {
                            frame.label_compact_sized(text(language, Key::NothingPlaying), 13.0);
                            frame.label_compact_sized(text(language, Key::AddLocalTrack), 10.0);
                        }
                    },
                );
            }
            frame.flex(1.0);
            playback_controls(app, frame);
            if width >= 960.0 {
                frame.size_next(168.0, 62.0);
                frame.column_ex(
                    &LayoutOpts {
                        width: 168.0,
                        height: 62.0,
                        gap: 3.0,
                        pad: 8.0,
                        bg: Color::rgba(255, 255, 255, 7),
                        radius: 15.0,
                        ..LayoutOpts::default()
                    },
                    |frame| {
                        frame.label_compact_sized(text(language, Key::Volume), 9.0);
                        if frame.slider("##volume", &mut app.volume, 0.0, 1.0) {
                            app.apply_volume();
                        }
                    },
                );
            }
        },
    );
}

fn playback_controls(app: &mut App, frame: &mut Frame) {
    frame.column_ex(
        &LayoutOpts {
            flex: 1.0,
            gap: 4.0,
            cross: Align::Stretch,
            ..LayoutOpts::default()
        },
        |frame| {
            frame.row_ex(
                &LayoutOpts {
                    height: 38.0,
                    gap: 7.0,
                    cross: Align::Center,
                    ..LayoutOpts::default()
                },
                |frame| {
                    frame.flex(1.0);
                    frame.spacer(0.0);
                    frame.size_next(36.0, 32.0);
                    if frame.selectable("◀", false) {
                        app.previous();
                    }
                    frame.size_next(48.0, 38.0);
                    if frame.button(if app.playback_state.is_playing() {
                        "Ⅱ"
                    } else {
                        "▶"
                    }) {
                        app.toggle_playback();
                    }
                    frame.size_next(36.0, 32.0);
                    if frame.selectable("▶|", false) {
                        app.next();
                    }
                    frame.flex(1.0);
                    frame.spacer(0.0);
                },
            );
            frame.row_ex(
                &LayoutOpts {
                    height: 24.0,
                    gap: 7.0,
                    cross: Align::Center,
                    ..LayoutOpts::default()
                },
                |frame| {
                    frame.label_compact_sized(&format_duration(app.position_ms), 9.5);
                    frame.flex(1.0);
                    if frame.slider("##timeline", &mut app.seek_ratio, 0.0, 1.0) {
                        app.commit_seek();
                    }
                    frame.label_compact_sized(&format_duration(app.duration_ms), 9.5);
                },
            );
        },
    );
}

fn ellipsize(value: &str, max_chars: usize) -> String {
    let count = value.chars().count();
    if count <= max_chars {
        return value.to_owned();
    }
    let keep = max_chars.saturating_sub(1);
    let mut result = value.chars().take(keep).collect::<String>();
    result.push('…');
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ellipsis_is_unicode_safe() {
        assert_eq!(ellipsize("春日长标题", 4), "春日长…");
        assert_eq!(ellipsize("short", 8), "short");
    }
}
