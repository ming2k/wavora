use crate::app::{App, View, VisualInspectorTab};
use crate::config::PlaylistDisplay;
use iris::{
    Align, Color, Frame, Icon, LayoutOpts, OverlayOpts, Rect, TableColumn, TableOpts, Theme,
};
use std::ffi::c_void;
use wavora_core::{PlaybackMode, PlaylistId, TrackId, format_duration};
use wavora_i18n::{Key, Language, LanguagePreference, text, visual_preset_text};
use wavora_ui::{
    InsightCard, InspectorTabs, PlayerControlButton, inspector_group, inspector_note,
    inspector_section, inspector_slider, player_control_button, theme as product_theme,
};
use wavora_visuals::{
    AtmosphereAudioResponse, AtmosphereFalloff, AtmosphereFieldKind, AtmospherePalette,
    AtmosphereSourceShape, MAX_ATMOSPHERE_SOURCES, PRESETS,
};

const ROOT_PAD: f32 = 18.0;
const GAP: f32 = 14.0;
const TOP_BAR_HEIGHT: f32 = 54.0;
const COMPACT_NAV_HEIGHT: f32 = 46.0;
const STATUS_HEIGHT: f32 = 40.0;
const PLAYER_HEIGHT: f32 = 112.0;
const TIMELINE_TRACK_THICKNESS: f32 = 3.0;
const TIMELINE_KNOB_SIZE: f32 = 10.0;
const TIMELINE_TIME_WIDTH: f32 = 42.0;
const TIMELINE_GAP: f32 = 8.0;
const SIDEBAR_MIN_WIDTH: f32 = 128.0;
const SIDEBAR_MAX_WIDTH: f32 = 190.0;
const SIDEBAR_PAD: f32 = 14.0;
const NAV_ICON_GAP: f32 = 8.0;
const VISUAL_INSPECTOR_WIDTH: f32 = 304.0;
const VISUAL_MIN_SIDE_STAGE_WIDTH: f32 = 480.0;
const VISUAL_STAGE_GAP: f32 = 14.0;
const VISUAL_COMPACT_STAGE_HEIGHT: f32 = 340.0;
const QUEUE_PANEL_WIDTH: f32 = 300.0;
const QUEUE_ITEM_HEIGHT: f32 = 76.0;
const QUEUE_ARTWORK_SIZE: f32 = 60.0;

#[must_use]
pub fn theme(preset: usize) -> Theme {
    let accent = PRESETS[preset % PRESETS.len()].accent;
    product_theme(accent)
}

pub fn build(
    app: &mut App,
    frame: &mut Frame,
    width: f32,
    height: f32,
    artwork: Option<*mut c_void>,
    playlist_artwork: &[(PlaylistId, *mut c_void)],
    queue_artwork: &[(TrackId, *mut c_void)],
) {
    frame.set_theme(theme(app.preset));
    let show_sidebar = width >= 760.0;
    let queue_progress = app.queue_panel_progress();
    let show_queue = queue_progress > f32::EPSILON;
    let status_visible = app.playback_error_message().is_some();
    let language = app.language();
    let sidebar_width = if show_sidebar {
        sidebar_intrinsic_width(app, frame, language)
    } else {
        0.0
    };
    let queue_width = QUEUE_PANEL_WIDTH * queue_progress;
    let queue_gap = GAP * queue_progress;
    let panel_width = (width
        - ROOT_PAD * 2.0
        - sidebar_width
        - queue_width
        - if show_sidebar { GAP } else { 0.0 }
        - queue_gap)
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
        let side_controls = uses_side_visual_layout(inner_width);
        let stage_width = if side_controls {
            side_visual_stage_width(inner_width)
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
                    gap: 0.0,
                    cross: Align::Stretch,
                    ..LayoutOpts::default()
                },
                |frame| {
                    if show_sidebar {
                        sidebar(app, frame, language);
                        frame.spacer(GAP);
                    }
                    main_content(
                        app,
                        frame,
                        panel_width,
                        content_height,
                        language,
                        playlist_artwork,
                    );
                    if show_queue {
                        frame.spacer(queue_gap);
                        queue(
                            app,
                            frame,
                            queue_width,
                            content_height,
                            language,
                            queue_artwork,
                        );
                    }
                },
            );
            player_bar(app, frame, width, language, artwork);
            playback_mode_toast(app, frame, width, height, language);
            transient_toast(app, frame, width, show_sidebar, status_visible);
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
                Icon::BookOpen,
                text(language, Key::Playlists),
                View::Playlists,
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
    let Some(message) = app.playback_error_message() else {
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
    frame.size_next(0.0, STATUS_HEIGHT);
    frame.row_ex(
        &LayoutOpts {
            height: STATUS_HEIGHT,
            pad: 10.0,
            cross: Align::Center,
            bg: Color::rgba(255, 96, 116, 22),
            radius: 12.0,
            ..LayoutOpts::default()
        },
        |frame| frame.label_compact_sized(&message, 11.5),
    );
}

fn transient_toast(
    app: &App,
    frame: &mut Frame,
    width: f32,
    show_sidebar: bool,
    status_visible: bool,
) {
    let Some(toast) = app.transient_toast() else {
        return;
    };
    let max_chars = if width >= 1_000.0 {
        72
    } else if width >= 700.0 {
        52
    } else if width >= 500.0 {
        38
    } else {
        24
    };
    let message = ellipsize(toast.message, max_chars);
    let max_width = (width - ROOT_PAD * 2.0).clamp(1.0, 480.0);
    let measured_width = frame.measure_text(&message, 11.5).width + 52.0;
    let toast_width = measured_width.max(160.0).min(max_width);
    let opacity = toast.opacity.clamp(0.0, 1.0);
    let alpha = metric_alpha(opacity * 255.0);
    let accent = PRESETS[app.preset % PRESETS.len()].accent;
    let tone = if toast.is_error {
        [255, 96, 116]
    } else {
        accent
    };
    let base = theme(app.preset);
    let toast_theme = base.with_fg(base.fg().with_alpha(alpha));
    let y = ROOT_PAD
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
    frame.set_theme(toast_theme);
    frame.layer(
        "transient-status-toast",
        Rect {
            x: ((width - toast_width) * 0.5).max(8.0),
            y: (y + toast.offset_y).max(8.0),
            w: toast_width,
            h: STATUS_HEIGHT,
        },
        &OverlayOpts {
            gap: 8.0,
            pad: 10.0,
            cross: Align::Center,
            bg: Color::rgba(10, 13, 18, metric_alpha(opacity * 244.0)),
            border: Color::rgba(tone[0], tone[1], tone[2], metric_alpha(opacity * 104.0)),
            border_width: 1.0,
            radius: 14.0,
            min_width: toast_width,
        },
        |frame| {
            frame.size_next(toast_width - 20.0, 20.0);
            frame.row_ex(
                &LayoutOpts {
                    width: toast_width - 20.0,
                    height: 20.0,
                    gap: 8.0,
                    cross: Align::Center,
                    ..LayoutOpts::default()
                },
                |frame| {
                    frame.set_theme(
                        toast_theme.with_fg(Color::rgba(tone[0], tone[1], tone[2], alpha)),
                    );
                    frame.icon(
                        if toast.is_error {
                            Icon::X
                        } else {
                            Icon::CheckCircle
                        },
                        16.0,
                    );
                    frame.set_theme(toast_theme);
                    frame.label_compact_sized(&message, 11.5);
                },
            );
        },
    );
    frame.set_theme(base);
}

fn sidebar(app: &mut App, frame: &mut Frame, language: Language) {
    frame.column_ex(
        &LayoutOpts {
            min_width: SIDEBAR_MIN_WIDTH,
            max_width: SIDEBAR_MAX_WIDTH,
            gap: 7.0,
            pad: SIDEBAR_PAD,
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
                Icon::BookOpen,
                text(language, Key::Playlists),
                View::Playlists,
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
            if app.scanning {
                frame.label_sized(text(language, Key::Scanning), 10.0);
            }
        },
    );
}

fn sidebar_intrinsic_width(app: &App, frame: &Frame, language: Language) -> f32 {
    let theme = frame.theme();
    let control_padding = theme.padding() * 2.0;
    let label_width =
        |label: &str, size: f32| frame.measure_text(label, size).width + control_padding;
    let nav_width = |label: &str| {
        theme.font_size()
            + NAV_ICON_GAP
            + frame.measure_text(label, theme.font_size()).width
            + control_padding
    };

    let local_tracks = format!("{} {}", app.tracks.len(), text(language, Key::LocalTracks));
    let favorites = format!(
        "{} {}",
        app.favorite_count(),
        text(language, Key::FavoriteTracks)
    );
    let mut widest_child = label_width(text(language, Key::YourSpace), 9.5)
        .max(nav_width(text(language, Key::Now)))
        .max(nav_width(text(language, Key::Library)))
        .max(nav_width(text(language, Key::Favorites)))
        .max(nav_width(text(language, Key::Playlists)))
        .max(nav_width(text(language, Key::VisualStage)))
        .max(label_width(text(language, Key::Collection), 9.5))
        .max(label_width(&local_tracks, theme.font_size()))
        .max(label_width(&favorites, theme.font_size()));
    if app.scanning {
        widest_child = widest_child.max(label_width(text(language, Key::Scanning), 10.0));
    }

    (widest_child + SIDEBAR_PAD * 2.0).clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH)
}

fn nav_item(app: &mut App, frame: &mut Frame, icon: Icon, label: &str, view: View) {
    frame.size_next(0.0, 40.0);
    if frame.selectable_icon(icon, label, app.view == view) {
        app.view = view;
    }
}

fn main_content(
    app: &mut App,
    frame: &mut Frame,
    width: f32,
    height: f32,
    language: Language,
    playlist_artwork: &[(PlaylistId, *mut c_void)],
) {
    let background = Color::rgba(7, 10, 15, content_surface_alpha(app.view));
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
            View::Playlists => playlists(app, frame, width, height, language, playlist_artwork),
            View::Lyrics => lyrics(app, frame, width, height, language),
            View::Visuals => visuals(app, frame, width, height, language),
            View::Settings => settings(app, frame, width, language),
        },
    );
}

fn content_surface_alpha(view: View) -> u8 {
    match view {
        View::Visuals => 92,
        View::Home => 128,
        View::Library | View::Favorites | View::Playlists | View::Lyrics | View::Settings => 148,
    }
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
            InsightCard::new(
                text(language, Key::LibraryCard),
                &format!("{} {}", app.tracks.len(), text(language, Key::Tracks)),
                text(language, Key::LocalArchive),
            )
            .show(frame);
            if width >= 720.0 {
                InsightCard::new(
                    text(language, Key::VisualCard),
                    visual_preset_text(language, app.preset).name,
                    visual_preset_text(language, app.preset).subtitle,
                )
                .show(frame);
                InsightCard::new(
                    text(language, Key::EngineCard),
                    "SYMPHONIA",
                    text(language, Key::SystemDecode),
                )
                .show(frame);
            }
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
        app.click_library_table_row(index, favorites_only);
    }
}

fn playlists(
    app: &mut App,
    frame: &mut Frame,
    width: f32,
    height: f32,
    language: Language,
    playlist_artwork: &[(PlaylistId, *mut c_void)],
) {
    if app.playlist_detail_open() && app.selected_playlist().is_some() {
        playlist_detail(app, frame, width, height, language);
    } else {
        playlist_collection(app, frame, width, height, language, playlist_artwork);
    }
}

fn playlist_collection(
    app: &mut App,
    frame: &mut Frame,
    width: f32,
    height: f32,
    language: Language,
    playlist_artwork: &[(PlaylistId, *mut c_void)],
) {
    let display = app.playlist_display();
    let mut requested_display = None;
    frame.row_ex(
        &LayoutOpts {
            height: 38.0,
            gap: 7.0,
            cross: Align::Center,
            ..LayoutOpts::default()
        },
        |frame| {
            frame.heading(text(language, Key::Playlists), 1);
            frame.flex(1.0);
            frame.spacer(0.0);
            if width >= 620.0 {
                frame.size_next(96.0, 34.0);
                if frame.selectable_icon(
                    Icon::Menu,
                    text(language, Key::ListView),
                    display == PlaylistDisplay::List,
                ) {
                    requested_display = Some(PlaylistDisplay::List);
                }
                frame.size_next(96.0, 34.0);
                if frame.selectable_icon(
                    Icon::Grid,
                    text(language, Key::CoverView),
                    display == PlaylistDisplay::Covers,
                ) {
                    requested_display = Some(PlaylistDisplay::Covers);
                }
            } else {
                frame.size_next(38.0, 34.0);
                if frame.icon_button_active(Icon::Menu, display == PlaylistDisplay::List) {
                    requested_display = Some(PlaylistDisplay::List);
                }
                frame.size_next(38.0, 34.0);
                if frame.icon_button_active(Icon::Grid, display == PlaylistDisplay::Covers) {
                    requested_display = Some(PlaylistDisplay::Covers);
                }
            }
        },
    );
    if let Some(requested_display) = requested_display {
        app.set_playlist_display(requested_display);
    }

    frame.row_ex(
        &LayoutOpts {
            height: 38.0,
            gap: 8.0,
            cross: Align::Center,
            ..LayoutOpts::default()
        },
        |frame| {
            frame.flex(1.0);
            frame.textfield(text(language, Key::NewPlaylist), &mut app.playlist_name);
            frame.size_next(100.0, 38.0);
            if frame.selectable(text(language, Key::CreatePlaylist), false) {
                app.create_playlist();
            }
        },
    );

    let available_playlists = app.playlists().to_vec();
    if available_playlists.is_empty() {
        frame.flex(1.0);
        frame.label_wrapped(text(language, Key::NoPlaylists), (width - 60.0).max(240.0));
        return;
    }

    let collection_height = (height - 118.0).max(160.0);
    match app.playlist_display() {
        PlaylistDisplay::List => {
            playlist_list_collection(app, frame, width, collection_height, &available_playlists);
        }
        PlaylistDisplay::Covers => {
            playlist_cover_collection(app, frame, width, collection_height, playlist_artwork);
        }
    }
}

fn playlist_list_collection(
    app: &mut App,
    frame: &mut Frame,
    width: f32,
    height: f32,
    playlists: &[wavora_core::Playlist],
) {
    let mut open_action = None;
    frame.size_next((width - 44.0).max(280.0), height);
    frame.scroll("playlist-list-collection", |frame| {
        frame.column_ex(
            &LayoutOpts {
                gap: 5.0,
                cross: Align::Stretch,
                ..LayoutOpts::default()
            },
            |frame| {
                for playlist in playlists {
                    frame.push_id(&playlist.id.to_string());
                    frame.size_next(0.0, 46.0);
                    if frame.selectable_icon(Icon::BookOpen, &ellipsize(&playlist.name, 56), false)
                    {
                        open_action = Some(playlist.id);
                    }
                    frame.pop_id();
                }
            },
        );
    });
    if let Some(playlist_id) = open_action {
        app.open_playlist(playlist_id);
    }
}

fn playlist_detail(app: &mut App, frame: &mut Frame, width: f32, height: f32, language: Language) {
    let Some(playlist_name) = app
        .selected_playlist()
        .map(|playlist| playlist.name.clone())
    else {
        app.close_playlist_detail();
        return;
    };
    let track_count = app.selected_playlist_tracks().len();
    let mut go_back = false;
    frame.row_ex(
        &LayoutOpts {
            height: 36.0,
            gap: 10.0,
            cross: Align::Center,
            ..LayoutOpts::default()
        },
        |frame| {
            if frame.link(text(language, Key::Playlists)) {
                go_back = true;
            }
            frame.label_compact_sized("/", 14.0);
            frame.label_compact_sized(
                &ellipsize(&playlist_name, if width >= 700.0 { 42 } else { 24 }),
                14.0,
            );
            frame.flex(1.0);
            frame.spacer(0.0);
            frame.label_sized(
                &format!("{} {}", track_count, text(language, Key::Tracks)),
                10.0,
            );
        },
    );
    if go_back {
        app.close_playlist_detail();
        return;
    }

    frame.row_ex(
        &LayoutOpts {
            height: 36.0,
            gap: 7.0,
            cross: Align::Center,
            ..LayoutOpts::default()
        },
        |frame| {
            frame.flex(1.0);
            frame.spacer(0.0);
            if frame.selectable(text(language, Key::AddCurrentTrack), false) {
                app.add_current_to_selected_playlist();
            }
            if frame.selectable(text(language, Key::RemoveFromPlaylist), false) {
                app.remove_selected_playlist_entry();
            }
            if width >= 700.0 {
                if frame.selectable(text(language, Key::MoveUp), false) {
                    app.move_selected_playlist_entry(-1);
                }
                if frame.selectable(text(language, Key::MoveDown), false) {
                    app.move_selected_playlist_entry(1);
                }
            }
            frame.size_next(38.0, 32.0);
            if frame.icon_button(Icon::Trash) {
                app.delete_selected_playlist();
            }
        },
    );

    let columns = [
        TableColumn {
            title: text(language, Key::Title),
            width: 0.0,
            align: Align::Start,
        },
        TableColumn {
            title: text(language, Key::Artist),
            width: 180.0,
            align: Align::Start,
        },
        TableColumn {
            title: text(language, Key::Duration),
            width: 72.0,
            align: Align::End,
        },
    ];
    let table_width = (width - 44.0).max(280.0);
    let table_height = (height - 106.0).max(140.0);
    let tracks = app.selected_playlist_tracks();
    frame.size_next(table_width, table_height);
    let result = frame.table(
        "playlist-table",
        &columns,
        tracks.len(),
        TableOpts {
            row_height: 46.0,
            show_header: true,
            selectable: true,
            zebra: true,
        },
        |row, column| {
            let Some(track) = tracks.get(row) else {
                return String::new();
            };
            match column {
                0 if track.available => ellipsize(&track.title, 42),
                0 => format!(
                    "{} · {}",
                    ellipsize(&track.title, 30),
                    text(language, Key::MissingTrack)
                ),
                1 => ellipsize(&track.artist, 22),
                2 => format_duration(track.duration_ms),
                _ => String::new(),
            }
        },
    );
    if result.clicked
        && let Some(row) = result.selected
    {
        app.click_playlist_table_row(row);
    }
}

#[allow(
    unsafe_code,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn playlist_cover_collection(
    app: &mut App,
    frame: &mut Frame,
    width: f32,
    viewport_height: f32,
    playlist_artwork: &[(PlaylistId, *mut c_void)],
) {
    const CARD_GAP: f32 = 10.0;
    const MIN_CARD_WIDTH: f32 = 148.0;
    const MAX_CARD_WIDTH: f32 = 176.0;

    let available_width = (width - 44.0).max(MIN_CARD_WIDTH);
    let mut columns = 1_usize;
    for candidate in 2_usize..=4 {
        let minimum = MIN_CARD_WIDTH * candidate as f32 + CARD_GAP * (candidate - 1) as f32;
        if available_width >= minimum {
            columns = candidate;
        }
    }
    let gaps = CARD_GAP * columns.saturating_sub(1) as f32;
    let card_width = ((available_width - gaps) / columns as f32).min(MAX_CARD_WIDTH);
    let cover_size = (card_width - 20.0).clamp(96.0, 156.0);
    let card_height = cover_size + 48.0;
    let cards = app.playlists().to_vec();
    let mut open_action = None;

    frame.size_next(available_width, viewport_height);
    frame.scroll("playlist-cover-gallery", |frame| {
        frame.column_ex(
            &LayoutOpts {
                gap: CARD_GAP,
                cross: Align::Stretch,
                ..LayoutOpts::default()
            },
            |frame| {
                for row in cards.chunks(columns) {
                    frame.row_ex(
                        &LayoutOpts {
                            height: card_height,
                            gap: CARD_GAP,
                            cross: Align::Start,
                            ..LayoutOpts::default()
                        },
                        |frame| {
                            for playlist in row {
                                frame.push_id(&playlist.id.to_string());
                                frame.size_next(card_width, card_height);
                                frame.column_ex(
                                    &LayoutOpts {
                                        width: card_width,
                                        height: card_height,
                                        gap: 5.0,
                                        pad: 10.0,
                                        cross: Align::Stretch,
                                        bg: Color::rgba(255, 255, 255, 9),
                                        radius: 17.0,
                                        ..LayoutOpts::default()
                                    },
                                    |frame| {
                                        frame.size_next(cover_size, cover_size);
                                        let cover_clicked = if let Some((_, texture)) =
                                            playlist_artwork.iter().find(|(playlist_id, _)| {
                                                *playlist_id == playlist.id
                                            }) {
                                            // SAFETY: ArtworkGallery owns the texture through the
                                            // complete Lens render for this frame.
                                            unsafe { frame.image_button(texture.cast()) }
                                        } else {
                                            frame.icon_button_badged(
                                                Icon::Radio,
                                                "",
                                                cover_size * 0.30,
                                                false,
                                            )
                                        };
                                        if cover_clicked {
                                            open_action = Some(playlist.id);
                                        }
                                        frame.size_next(0.0, 26.0);
                                        frame.row_ex(
                                            &LayoutOpts {
                                                height: 26.0,
                                                cross: Align::Center,
                                                ..LayoutOpts::default()
                                            },
                                            |frame| {
                                                frame.flex(1.0);
                                                frame.spacer(0.0);
                                                frame.label_compact_sized(
                                                    &ellipsize(&playlist.name, 22),
                                                    12.0,
                                                );
                                                frame.flex(1.0);
                                                frame.spacer(0.0);
                                            },
                                        );
                                    },
                                );
                                frame.pop_id();
                            }
                        },
                    );
                }
            },
        );
    });
    if let Some(playlist_id) = open_action {
        app.open_playlist(playlist_id);
    }
}

fn lyrics(app: &mut App, frame: &mut Frame, width: f32, height: f32, language: Language) {
    frame.row_ex(
        &LayoutOpts {
            height: 38.0,
            gap: 8.0,
            cross: Align::Center,
            ..LayoutOpts::default()
        },
        |frame| {
            frame.heading(text(language, Key::Lyrics), 1);
            frame.flex(1.0);
            frame.size_next(132.0, 34.0);
            if frame.selectable(text(language, Key::ReloadLyrics), false) {
                app.refresh_current_lyrics();
            }
        },
    );
    if let Some(track) = app.current_track() {
        frame.label_sized(&ellipsize(&track.title, 64), 16.0);
        frame.label_sized(&ellipsize(&track.artist, 48), 10.5);
    }
    let active = app.active_lyric_cues();
    let source = app
        .lyrics_path()
        .map(|path| ellipsize(&path.display().to_string(), 96));
    let Some(document) = app.lyrics() else {
        frame.flex(1.0);
        frame.label_sized(text(language, Key::NoLyrics), 16.0);
        frame.label_wrapped_sized(
            text(language, Key::LyricsSidecarHint),
            11.0,
            (width - 60.0).max(240.0),
        );
        return;
    };
    if let Some(source) = source {
        frame.label_sized(&source, 9.0);
    }
    for cue in active.iter().filter_map(|index| document.cues.get(*index)) {
        let accent = PRESETS[app.preset % PRESETS.len()].accent;
        let current_height = lyric_cue_height(cue, true);
        frame.size_next(0.0, current_height);
        frame.column_ex(
            &LayoutOpts {
                height: current_height,
                gap: 4.0,
                pad: 12.0,
                cross: Align::Stretch,
                bg: Color::rgba(accent[0], accent[1], accent[2], 40),
                radius: 14.0,
                ..LayoutOpts::default()
            },
            |frame| {
                if let Some(track) = document
                    .tracks
                    .iter()
                    .find(|track| track.id == cue.track_id)
                {
                    frame.label_sized(track.label.as_deref().unwrap_or(&track.role), 8.5);
                }
                lyric_cue_texts(frame, cue, (width - 90.0).max(220.0), true);
            },
        );
    }
    frame.separator();
    frame.size_next((width - 44.0).max(280.0), (height - 220.0).max(160.0));
    frame.scroll("lyrics-scroll", |frame| {
        for (index, cue) in document.cues.iter().enumerate() {
            let is_active = active.contains(&index);
            let accent = PRESETS[app.preset % PRESETS.len()].accent;
            let line_height = lyric_cue_height(cue, false);
            frame.size_next(0.0, line_height);
            frame.column_ex(
                &LayoutOpts {
                    height: line_height,
                    gap: 4.0,
                    pad: 10.0,
                    cross: Align::Stretch,
                    bg: if is_active {
                        Color::rgba(accent[0], accent[1], accent[2], 34)
                    } else {
                        Color::rgba(255, 255, 255, 4)
                    },
                    radius: 12.0,
                    ..LayoutOpts::default()
                },
                |frame| {
                    lyric_cue_texts(frame, cue, (width - 86.0).max(220.0), is_active);
                },
            );
            frame.spacer(4.0);
        }
    });
}

fn lyric_cue_height(cue: &wavora_core::LyricCue, current: bool) -> f32 {
    let alternatives = cue.texts.len().saturating_sub(1);
    let alternatives = f32::from(u16::try_from(alternatives).unwrap_or(16));
    if current {
        70.0 + alternatives * 22.0
    } else {
        50.0 + alternatives * 20.0
    }
}

fn lyric_cue_texts(frame: &mut Frame, cue: &wavora_core::LyricCue, width: f32, active: bool) {
    if let Some(original) = cue.original_text() {
        frame.label_wrapped_sized(&original.text, if active { 18.0 } else { 13.0 }, width);
    }
    for text_variant in cue.texts.iter().filter(|text| text.kind != "original") {
        frame.label_wrapped_sized(&text_variant.text, 10.5, width);
    }
}

fn visuals(app: &mut App, frame: &mut Frame, width: f32, height: f32, language: Language) {
    let inner_width = (width - 44.0).max(1.0);
    let inner_height = (height - 44.0).max(1.0);
    if uses_side_visual_layout(inner_width) {
        let stage_width = side_visual_stage_width(inner_width);
        frame.row_ex(
            &LayoutOpts {
                flex: 1.0,
                gap: VISUAL_STAGE_GAP,
                cross: Align::Stretch,
                ..LayoutOpts::default()
            },
            |frame| {
                visual_stage(app, frame, stage_width, inner_height, language);
                visual_controls(app, frame, VISUAL_INSPECTOR_WIDTH, language);
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

fn uses_side_visual_layout(inner_width: f32) -> bool {
    inner_width >= VISUAL_MIN_SIDE_STAGE_WIDTH + VISUAL_INSPECTOR_WIDTH + VISUAL_STAGE_GAP
}

fn side_visual_stage_width(inner_width: f32) -> f32 {
    (inner_width - VISUAL_INSPECTOR_WIDTH - VISUAL_STAGE_GAP).max(1.0)
}

fn stacked_visual_stage_height(inner_height: f32) -> f32 {
    (inner_height * 0.38).clamp(190.0, 260.0).min(inner_height)
}

fn uses_compact_visual_stage(height: f32) -> bool {
    height < VISUAL_COMPACT_STAGE_HEIGHT
}

fn visual_stage(app: &App, frame: &mut Frame, width: f32, height: f32, language: Language) {
    let copy = visual_preset_text(language, app.preset);
    let features = app.live_audio_features();
    let metric_motion = app.audio_metric_snapshot();
    let is_live = app.playback_state.is_playing();
    let compact = uses_compact_visual_stage(height);
    let stage_pad = if compact { 14.0 } else { 18.0 };
    frame.column_ex(
        &LayoutOpts {
            flex: 0.0,
            width,
            height,
            gap: if compact { 5.0 } else { 8.0 },
            pad: stage_pad,
            cross: Align::Stretch,
            bg: Color::rgba(3, 6, 11, 52),
            radius: 20.0,
            ..LayoutOpts::default()
        },
        |frame| {
            let accent = PRESETS[app.preset % PRESETS.len()].accent;
            frame.row_ex(
                &LayoutOpts {
                    gap: 8.0,
                    cross: Align::Center,
                    ..LayoutOpts::default()
                },
                |frame| {
                    frame.label_compact_sized(text(language, Key::VisualPreview), 8.5);
                    frame.flex(1.0);
                    frame.spacer(0.0);
                    frame.row_ex(
                        &LayoutOpts {
                            height: 24.0,
                            pad: 6.0,
                            cross: Align::Center,
                            bg: Color::rgba(accent[0], accent[1], accent[2], 28),
                            radius: 12.0,
                            ..LayoutOpts::default()
                        },
                        |frame| {
                            frame.label_compact_sized(
                                if is_live {
                                    text(language, Key::Live)
                                } else {
                                    text(language, Key::WaitingForAudio)
                                },
                                8.0,
                            );
                        },
                    );
                },
            );
            frame.spacer(2.0);
            frame.label_sized(
                copy.name,
                if compact {
                    20.0
                } else if width >= 430.0 {
                    28.0
                } else {
                    22.0
                },
            );
            frame.label_wrapped_sized(
                copy.subtitle,
                if compact { 9.5 } else { 11.0 },
                (width - stage_pad * 2.0).max(120.0),
            );
            if !compact {
                frame.column_ex(
                    &LayoutOpts {
                        gap: 2.0,
                        pad: 10.0,
                        cross: Align::Stretch,
                        bg: Color::rgba(255, 255, 255, 7),
                        radius: 12.0,
                        ..LayoutOpts::default()
                    },
                    |frame| {
                        frame.label_wrapped_sized(copy.response, 9.5, (width - 56.0).max(100.0));
                    },
                );
            }
            frame.flex(1.0);
            frame.spacer(0.0);
            if compact {
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
                    (width - stage_pad * 2.0).max(120.0),
                );
            } else {
                visual_metrics(
                    frame,
                    features,
                    metric_motion,
                    app.preset,
                    (width - 36.0).max(1.0),
                    language,
                );
            }
        },
    );
}

fn visual_metrics(
    frame: &mut Frame,
    features: wavora_media::AudioFeatures,
    motion: wavora_visuals::AudioMetricSnapshot,
    preset: usize,
    width: f32,
    language: Language,
) {
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
    let colors = metric_palette(preset);
    let card_width = ((width - 18.0) / 4.0).max(1.0);
    let track_width = (card_width - 16.0).max(1.0);
    frame.row_ex(
        &LayoutOpts {
            height: 68.0,
            gap: 6.0,
            cross: Align::Stretch,
            ..LayoutOpts::default()
        },
        |frame| {
            for (index, (label, value)) in labels.into_iter().zip(values).enumerate() {
                let level = motion.levels[index].clamp(0.0, 1.0);
                let pulse = motion.pulses[index].clamp(0.0, 1.0);
                let color = colors[index];
                frame.flex(1.0);
                frame.column_ex(
                    &LayoutOpts {
                        flex: 1.0,
                        height: 68.0,
                        gap: 3.0,
                        pad: 8.0,
                        bg: Color::rgba(
                            color[0],
                            color[1],
                            color[2],
                            metric_alpha(10.0 + pulse * 13.0),
                        ),
                        radius: 11.0,
                        ..LayoutOpts::default()
                    },
                    |frame| {
                        frame.label_compact_sized(label, 7.5);
                        frame.label_compact_sized(&value, 10.0);
                        frame.flex(1.0);
                        frame.row_ex(
                            &LayoutOpts {
                                width: track_width,
                                height: 7.0,
                                bg: Color::rgba(255, 255, 255, 16),
                                radius: 3.5,
                                ..LayoutOpts::default()
                            },
                            |frame| {
                                let fill_width = track_width * level;
                                if fill_width >= 0.5 {
                                    frame.row_ex(
                                        &LayoutOpts {
                                            width: fill_width,
                                            height: 7.0,
                                            bg: Color::rgba(
                                                color[0],
                                                color[1],
                                                color[2],
                                                metric_alpha(168.0 + pulse * 87.0),
                                            ),
                                            radius: 3.5,
                                            ..LayoutOpts::default()
                                        },
                                        |_| {},
                                    );
                                }
                            },
                        );
                    },
                );
            }
        },
    );
}

fn metric_palette(preset: usize) -> [[u8; 3]; 4] {
    let preset = PRESETS[preset % PRESETS.len()];
    [
        preset.accent,
        mix_rgb(preset.accent, preset.secondary, 36),
        preset.secondary,
        mix_rgb(preset.secondary, [255, 255, 255], 24),
    ]
}

fn mix_rgb(left: [u8; 3], right: [u8; 3], right_percent: u16) -> [u8; 3] {
    let right_percent = right_percent.min(100);
    let left_percent = 100 - right_percent;
    std::array::from_fn(|index| {
        let mixed = u16::from(left[index]) * left_percent + u16::from(right[index]) * right_percent;
        u8::try_from((mixed + 50) / 100).unwrap_or(255)
    })
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn metric_alpha(value: f32) -> u8 {
    value.clamp(0.0, 255.0).round() as u8
}

fn visual_controls(app: &mut App, frame: &mut Frame, width: f32, language: Language) {
    frame.column_ex(
        &LayoutOpts {
            flex: 1.0,
            width,
            gap: 10.0,
            pad: 14.0,
            cross: Align::Stretch,
            bg: Color::rgba(5, 8, 13, 255),
            radius: 20.0,
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
            let active_tab = match app.visual_inspector_tab {
                VisualInspectorTab::Composition => 0,
                VisualInspectorTab::Atmosphere => 1,
            };
            let tab_rail_width = (width - 28.0).max(1.0);
            let labels = [
                text(language, Key::Composition),
                text(language, Key::Atmosphere),
            ];
            let active_tab =
                InspectorTabs::new("visual-inspector-tabs", &labels, active_tab, tab_rail_width)
                    .show(frame);
            app.visual_inspector_tab = if active_tab == 1 {
                VisualInspectorTab::Atmosphere
            } else {
                VisualInspectorTab::Composition
            };
            frame.flex(1.0);
            match app.visual_inspector_tab {
                VisualInspectorTab::Composition => {
                    frame.scroll("visual-composition-scroll", |frame| {
                        composition_controls(app, frame, width, language);
                    });
                }
                VisualInspectorTab::Atmosphere => {
                    frame.scroll("visual-atmosphere-scroll", |frame| {
                        atmosphere_controls(app, frame, width, language);
                    });
                }
            }
        },
    );
}

fn composition_controls(app: &mut App, frame: &mut Frame, width: f32, language: Language) {
    inspector_section(frame, text(language, Key::VisualPresets), |frame| {
        let preset_names = (0..PRESETS.len())
            .map(|index| visual_preset_text(language, index).name)
            .collect::<Vec<_>>();
        let mut selected = i32::try_from(app.preset).unwrap_or_default();
        if frame.dropdown(
            text(language, Key::Composition),
            &mut selected,
            &preset_names,
        ) {
            app.set_preset(usize::try_from(selected).unwrap_or_default());
        }
        let copy = visual_preset_text(language, app.preset);
        frame.label_wrapped_sized(copy.subtitle, 9.5, (width - 52.0).max(120.0));
    });
    frame.spacer(10.0);
    inspector_section(frame, text(language, Key::ResponseTuning), |frame| {
        let copy = visual_preset_text(language, app.preset);
        frame.label_wrapped_sized(copy.response, 8.5, (width - 52.0).max(120.0));
        let mut changed = false;
        changed |= inspector_slider(
            frame,
            text(language, Key::Intensity),
            "##composition-intensity",
            &mut app.visual_tuning.intensity,
            0.45,
            1.75,
        );
        changed |= inspector_slider(
            frame,
            text(language, Key::Motion),
            "##composition-motion",
            &mut app.visual_tuning.motion,
            0.35,
            1.65,
        );
        changed |= inspector_slider(
            frame,
            text(language, Key::Depth),
            "##composition-depth",
            &mut app.visual_tuning.depth,
            0.50,
            1.50,
        );
        changed |= inspector_slider(
            frame,
            text(language, Key::Glow),
            "##composition-glow",
            &mut app.visual_tuning.glow,
            0.25,
            1.50,
        );
        if changed {
            app.apply_visual_tuning();
        }
    });
    frame.spacer(10.0);
    inspector_note(frame, text(language, Key::VisualFootnote), width);
}

fn atmosphere_controls(app: &mut App, frame: &mut Frame, width: f32, language: Language) {
    let layer_changed = inspector_section(frame, text(language, Key::SceneLayers), |frame| {
        let mut changed = false;
        changed |= frame.checkbox(
            text(language, Key::AtmosphereEnabled),
            &mut app.atmosphere.enabled,
        );
        changed |= frame.checkbox(
            text(language, Key::CompositionVisible),
            &mut app.atmosphere.composition_visible,
        );
        changed
    });
    if layer_changed {
        app.apply_atmosphere();
    }

    frame.spacer(10.0);
    inspector_section(frame, text(language, Key::Material), |frame| {
        material_controls(app, frame, language);
    });

    frame.spacer(10.0);
    inspector_section(frame, text(language, Key::LightSources), |frame| {
        light_source_controls(app, frame, width, language);
    });

    frame.spacer(10.0);
    inspector_note(frame, text(language, Key::AtmosphereHint), width);
}

fn material_controls(app: &mut App, frame: &mut Frame, language: Language) {
    let changed = {
        let field = &mut app.atmosphere.field;
        let mut changed = false;
        let mut kind = match field.kind {
            AtmosphereFieldKind::None => 0,
            AtmosphereFieldKind::Watercolor => 1,
            AtmosphereFieldKind::Caustics => 2,
        };
        if frame.dropdown(
            text(language, Key::MaterialField),
            &mut kind,
            &[
                text(language, Key::NoMaterial),
                text(language, Key::Watercolor),
                text(language, Key::Caustics),
            ],
        ) {
            field.kind = match kind {
                1 => AtmosphereFieldKind::Watercolor,
                2 => AtmosphereFieldKind::Caustics,
                _ => AtmosphereFieldKind::None,
            };
            changed = true;
        }
        if field.kind != AtmosphereFieldKind::None {
            changed |= inspector_slider(
                frame,
                text(language, Key::Strength),
                "##material-strength",
                &mut field.intensity,
                0.0,
                1.5,
            );
            changed |= inspector_slider(
                frame,
                text(language, Key::TextureScale),
                "##material-scale",
                &mut field.scale,
                0.45,
                2.2,
            );
            changed |= inspector_slider(
                frame,
                text(language, Key::FieldMotion),
                "##material-motion",
                &mut field.motion,
                0.0,
                1.0,
            );
            if frame.button(text(language, Key::NewVariation)) {
                field.seed = field.seed.wrapping_mul(0x9E37_79B9).rotate_left(13) ^ 0xA341_316C;
                changed = true;
            }
            let mut palette = match field.palette {
                AtmospherePalette::Preset => 0,
                AtmospherePalette::Custom => 1,
            };
            if frame.dropdown(
                text(language, Key::Palette),
                &mut palette,
                &[
                    text(language, Key::FollowPreset),
                    text(language, Key::CustomColor),
                ],
            ) {
                field.palette = if palette == 0 {
                    AtmospherePalette::Preset
                } else {
                    AtmospherePalette::Custom
                };
                changed = true;
            }
            if field.palette == AtmospherePalette::Custom {
                changed |= inspector_slider(
                    frame,
                    text(language, Key::Hue),
                    "##material-hue",
                    &mut field.hue,
                    0.0,
                    1.0,
                );
                changed |= inspector_slider(
                    frame,
                    text(language, Key::Saturation),
                    "##material-saturation",
                    &mut field.saturation,
                    0.0,
                    1.0,
                );
            }
        }
        changed
    };
    if changed {
        app.apply_atmosphere();
    }
}

fn light_source_controls(app: &mut App, frame: &mut Frame, width: f32, language: Language) {
    if !app.atmosphere.sources.is_empty() {
        let source_names = (0..app.atmosphere.sources.len())
            .map(|index| format!("{} {}", text(language, Key::LightSource), index + 1))
            .collect::<Vec<_>>();
        let source_labels = source_names.iter().map(String::as_str).collect::<Vec<_>>();
        let mut selected = i32::try_from(app.selected_atmosphere_source).unwrap_or_default();
        if frame.dropdown(
            text(language, Key::LightSource),
            &mut selected,
            &source_labels,
        ) {
            app.selected_atmosphere_source = usize::try_from(selected)
                .unwrap_or_default()
                .min(app.atmosphere.sources.len() - 1);
        }
    }

    let mut add_source = false;
    let mut remove_source = false;
    frame.row_ex(
        &LayoutOpts {
            gap: 8.0,
            cross: Align::Stretch,
            ..LayoutOpts::default()
        },
        |frame| {
            if app.atmosphere.sources.len() < MAX_ATMOSPHERE_SOURCES {
                frame.flex(1.0);
                add_source = frame.button(text(language, Key::AddLight));
            }
            if !app.atmosphere.sources.is_empty() {
                frame.flex(1.0);
                remove_source = frame.button(text(language, Key::RemoveLight));
            }
        },
    );
    if add_source {
        app.add_atmosphere_source();
    } else if remove_source {
        app.remove_selected_atmosphere_source();
    }
    if app.atmosphere.sources.is_empty() {
        frame.label_wrapped_sized(
            text(language, Key::AtmosphereHint),
            8.5,
            (width - 52.0).max(120.0),
        );
        return;
    }

    let index = app
        .selected_atmosphere_source
        .min(app.atmosphere.sources.len() - 1);
    let changed = {
        let source = &mut app.atmosphere.sources[index];
        let mut changed = false;

        inspector_group(frame, text(language, Key::Placement));
        let mut shape = match source.shape {
            AtmosphereSourceShape::Circle => 0,
            AtmosphereSourceShape::Oval => 1,
            AtmosphereSourceShape::Beam => 2,
        };
        if frame.dropdown(
            text(language, Key::SourceShape),
            &mut shape,
            &[
                text(language, Key::Circle),
                text(language, Key::Oval),
                text(language, Key::Beam),
            ],
        ) {
            source.shape = match shape {
                1 => AtmosphereSourceShape::Oval,
                2 => AtmosphereSourceShape::Beam,
                _ => AtmosphereSourceShape::Circle,
            };
            changed = true;
        }
        changed |= inspector_slider(
            frame,
            text(language, Key::Horizontal),
            "##source-x",
            &mut source.x,
            -2.0,
            3.0,
        );
        changed |= inspector_slider(
            frame,
            text(language, Key::Vertical),
            "##source-y",
            &mut source.y,
            -2.0,
            3.0,
        );
        changed |= inspector_slider(
            frame,
            text(language, Key::Radius),
            "##source-radius",
            &mut source.radius,
            0.05,
            2.0,
        );
        if source.shape != AtmosphereSourceShape::Circle {
            changed |= inspector_slider(
                frame,
                text(language, Key::AspectRatio),
                "##source-aspect",
                &mut source.aspect,
                1.0,
                4.0,
            );
            changed |= inspector_slider(
                frame,
                text(language, Key::Rotation),
                "##source-rotation",
                &mut source.rotation,
                -0.5,
                0.5,
            );
        }

        inspector_group(frame, text(language, Key::Appearance));
        let mut palette = match source.palette {
            AtmospherePalette::Preset => 0,
            AtmospherePalette::Custom => 1,
        };
        if frame.dropdown(
            text(language, Key::Palette),
            &mut palette,
            &[
                text(language, Key::FollowPreset),
                text(language, Key::CustomColor),
            ],
        ) {
            source.palette = if palette == 0 {
                AtmospherePalette::Preset
            } else {
                AtmospherePalette::Custom
            };
            changed = true;
        }
        let mut falloff = match source.falloff {
            AtmosphereFalloff::Diffuse => 0,
            AtmosphereFalloff::Focused => 1,
            AtmosphereFalloff::Halo => 2,
        };
        if frame.dropdown(
            text(language, Key::Falloff),
            &mut falloff,
            &[
                text(language, Key::Diffuse),
                text(language, Key::Focused),
                text(language, Key::Halo),
            ],
        ) {
            source.falloff = match falloff {
                1 => AtmosphereFalloff::Focused,
                2 => AtmosphereFalloff::Halo,
                _ => AtmosphereFalloff::Diffuse,
            };
            changed = true;
        }
        changed |= inspector_slider(
            frame,
            text(language, Key::Strength),
            "##source-strength",
            &mut source.intensity,
            0.0,
            2.0,
        );
        if source.palette == AtmospherePalette::Custom {
            changed |= inspector_slider(
                frame,
                text(language, Key::Hue),
                "##source-hue",
                &mut source.hue,
                0.0,
                1.0,
            );
            changed |= inspector_slider(
                frame,
                text(language, Key::Saturation),
                "##source-saturation",
                &mut source.saturation,
                0.0,
                1.0,
            );
        }

        inspector_group(frame, text(language, Key::MotionAndAudio));
        changed |= inspector_slider(
            frame,
            text(language, Key::Drift),
            "##source-drift",
            &mut source.drift,
            0.0,
            0.18,
        );
        let mut audio_response = match source.audio_response {
            AtmosphereAudioResponse::None => 0,
            AtmosphereAudioResponse::Energy => 1,
            AtmosphereAudioResponse::Bass => 2,
            AtmosphereAudioResponse::Mid => 3,
            AtmosphereAudioResponse::Treble => 4,
            AtmosphereAudioResponse::Onset => 5,
        };
        if frame.dropdown(
            text(language, Key::AudioResponse),
            &mut audio_response,
            &[
                text(language, Key::NoResponse),
                text(language, Key::Energy),
                text(language, Key::Bass),
                text(language, Key::Midrange),
                text(language, Key::Treble),
                text(language, Key::Onset),
            ],
        ) {
            source.audio_response = match audio_response {
                1 => AtmosphereAudioResponse::Energy,
                2 => AtmosphereAudioResponse::Bass,
                3 => AtmosphereAudioResponse::Mid,
                4 => AtmosphereAudioResponse::Treble,
                5 => AtmosphereAudioResponse::Onset,
                _ => AtmosphereAudioResponse::None,
            };
            changed = true;
        }
        if source.audio_response != AtmosphereAudioResponse::None {
            changed |= inspector_slider(
                frame,
                text(language, Key::ScaleResponse),
                "##source-audio-scale",
                &mut source.audio_scale,
                0.0,
                0.8,
            );
            changed |= inspector_slider(
                frame,
                text(language, Key::StrengthResponse),
                "##source-audio-strength",
                &mut source.audio_intensity,
                0.0,
                1.5,
            );
        }
        changed
    };
    if changed {
        app.apply_atmosphere();
    }
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
    let mut playback_mode = match app.playback_mode {
        PlaybackMode::Sequential => 0,
        PlaybackMode::RepeatOne => 1,
        PlaybackMode::Shuffle => 2,
    };
    let playback_modes = [
        text(language, Key::Sequential),
        text(language, Key::RepeatOne),
        text(language, Key::Shuffle),
    ];
    if frame.dropdown(
        text(language, Key::PlaybackMode),
        &mut playback_mode,
        &playback_modes,
    ) {
        app.set_playback_mode(match playback_mode {
            1 => PlaybackMode::RepeatOne,
            2 => PlaybackMode::Shuffle,
            _ => PlaybackMode::Sequential,
        });
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
    frame.label_wrapped(
        &format!(
            "{}: {}",
            text(language, Key::StateFile),
            app.state_path().display()
        ),
        (width - 60.0).max(260.0),
    );
    frame.label_wrapped(
        &format!(
            "{}: {}",
            text(language, Key::FavoritesFile),
            app.user_data_path().display()
        ),
        (width - 60.0).max(260.0),
    );
    frame.label_wrapped(
        &format!(
            "{}: {}",
            text(language, Key::CatalogFile),
            app.catalog_path().display()
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

fn queue(
    app: &mut App,
    frame: &mut Frame,
    width: f32,
    height: f32,
    language: Language,
    artwork: &[(TrackId, *mut c_void)],
) {
    frame.column_ex(
        &LayoutOpts {
            width,
            gap: 8.0,
            pad: 16.0,
            cross: Align::Stretch,
            bg: Color::rgba(9, 12, 17, 208),
            radius: 22.0,
            ..LayoutOpts::default()
        },
        |frame| {
            frame.row_ex(
                &LayoutOpts {
                    cross: Align::Center,
                    ..LayoutOpts::default()
                },
                |frame| {
                    frame.heading(text(language, Key::UpNext), 2);
                    frame.flex(1.0);
                    frame.label_sized(text(language, Key::Queue), 9.0);
                },
            );
            frame.separator();
            frame.size_next(0.0, (height - 70.0).max(150.0));
            frame.scroll("queue-scroll", |frame| {
                if app.tracks.is_empty() {
                    frame.label_sized(text(language, Key::EmptyQueue), 11.0);
                }
                for (queue_position, index) in app.queue_items() {
                    let track = &app.tracks[index];
                    let track_id = track.id;
                    let title = track.title.clone();
                    let metadata = format!("{}  ·  {}", track.artist, track.album);
                    let texture = artwork.iter().find_map(|(candidate, texture)| {
                        (*candidate == track_id).then_some(*texture)
                    });
                    let active = app.playback_queue_position() == Some(queue_position);
                    if queue_item(
                        app,
                        frame,
                        queue_position,
                        &title,
                        &metadata,
                        texture,
                        active,
                    ) {
                        app.play_queue_position(queue_position);
                    }
                }
            });
        },
    );
}

fn queue_item(
    app: &App,
    frame: &mut Frame,
    queue_position: usize,
    title: &str,
    metadata: &str,
    artwork: Option<*mut c_void>,
    active: bool,
) -> bool {
    let accent = PRESETS[app.preset % PRESETS.len()].accent;
    let base = theme(app.preset);
    let mut clicked = false;
    frame.push_id(&format!("queue-item-{queue_position}"));
    frame.size_next(0.0, QUEUE_ITEM_HEIGHT);
    frame.row_ex(
        &LayoutOpts {
            height: QUEUE_ITEM_HEIGHT,
            gap: 10.0,
            pad: 8.0,
            cross: Align::Center,
            bg: if active {
                Color::rgba(accent[0], accent[1], accent[2], 18)
            } else {
                Color::TRANSPARENT
            },
            radius: 12.0,
            ..LayoutOpts::default()
        },
        |frame| {
            album_art(app, frame, artwork, QUEUE_ARTWORK_SIZE);
            frame.column_ex(
                &LayoutOpts {
                    flex: 1.0,
                    gap: 5.0,
                    ..LayoutOpts::default()
                },
                |frame| {
                    frame.set_theme(base.with_fg(if active {
                        Color::rgba(accent[0], accent[1], accent[2], 255)
                    } else {
                        base.fg()
                    }));
                    clicked |= frame.link(&ellipsize(title, 25));
                    frame.set_theme(
                        base.with_fg(Color::rgba(174, 177, 184, 255))
                            .with_font_size(11.0),
                    );
                    frame.row_ex(
                        &LayoutOpts {
                            gap: 5.0,
                            cross: Align::Center,
                            ..LayoutOpts::default()
                        },
                        |frame| {
                            frame.icon(Icon::Play, 14.0);
                            clicked |= frame.link(&ellipsize(metadata, 29));
                        },
                    );
                    frame.set_theme(base);
                },
            );
        },
    );
    frame.pop_id();
    clicked
}

fn player_bar(
    app: &mut App,
    frame: &mut Frame,
    width: f32,
    language: Language,
    artwork: Option<*mut c_void>,
) {
    frame.size_next(0.0, PLAYER_HEIGHT);
    frame.row_ex(
        &LayoutOpts {
            height: PLAYER_HEIGHT,
            gap: 12.0,
            pad: 10.0,
            cross: Align::Center,
            bg: Color::rgba(7, 9, 13, 238),
            radius: 24.0,
            ..LayoutOpts::default()
        },
        |frame| {
            let track_width = if width >= 1_280.0 {
                300.0
            } else if width >= 900.0 {
                260.0
            } else if width >= 720.0 {
                220.0
            } else {
                180.0
            };
            frame.size_next(track_width, 60.0);
            now_playing_panel(
                app,
                frame,
                track_width,
                60.0,
                artwork,
                language,
                width >= 720.0,
            );
            let auxiliary_width = if width >= 1_280.0 {
                180.0
            } else if width >= 760.0 {
                174.0
            } else {
                160.0
            };
            let controls_width =
                (width - ROOT_PAD * 2.0 - 20.0 - track_width - auxiliary_width - 24.0).max(220.0);
            playback_controls(app, frame, controls_width);
            auxiliary_controls(app, frame, auxiliary_width, language);
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn now_playing_panel(
    app: &mut App,
    frame: &mut Frame,
    width: f32,
    height: f32,
    artwork: Option<*mut c_void>,
    language: Language,
    show_favorite: bool,
) {
    let current = app.current_track().map(|track| {
        (
            ellipsize(&track.title, if width >= 280.0 { 30 } else { 22 }),
            ellipsize(&track.artist, if width >= 280.0 { 24 } else { 18 }),
            track.codec.clone(),
            track.favorite,
        )
    });
    frame.row_ex(
        &LayoutOpts {
            width,
            height,
            flex: if width == 0.0 { 1.0 } else { 0.0 },
            gap: 9.0,
            pad: 6.0,
            cross: Align::Center,
            bg: Color::rgba(255, 255, 255, 9),
            radius: 15.0,
            ..LayoutOpts::default()
        },
        |frame| {
            album_art(app, frame, artwork, height - 12.0);
            frame.flex(1.0);
            frame.column_ex(
                &LayoutOpts {
                    flex: 1.0,
                    gap: 3.0,
                    ..LayoutOpts::default()
                },
                |frame| {
                    if let Some((title, artist, codec, _)) = current.as_ref() {
                        frame.label_compact_sized(title, 13.0);
                        frame.label_compact_sized(&format!("{artist}  ·  {codec}"), 9.5);
                    } else {
                        frame.label_compact_sized(text(language, Key::NothingPlaying), 12.5);
                        frame.label_compact_sized(text(language, Key::AddLocalTrack), 9.5);
                    }
                },
            );
            if show_favorite && let Some((_, _, _, favorite)) = current {
                frame.size_next(44.0, 40.0);
                if frame.icon_button_badged(Icon::Star, "", 26.0, favorite) {
                    app.toggle_current_favorite();
                }
            }
        },
    );
}

#[allow(unsafe_code)]
fn album_art(app: &App, frame: &mut Frame, artwork: Option<*mut c_void>, size: f32) {
    frame.size_next(size, size);
    if let Some(artwork) = artwork {
        // SAFETY: ArtworkCache owns this texture for at least the complete
        // build/paint/render cycle and only replaces it between frames.
        unsafe { frame.image(artwork.cast(), size, size) };
        return;
    }
    let accent = PRESETS[app.preset % PRESETS.len()].accent;
    frame.column_ex(
        &LayoutOpts {
            width: size,
            height: size,
            cross: Align::Center,
            bg: Color::rgba(accent[0], accent[1], accent[2], 34),
            radius: 9.0,
            ..LayoutOpts::default()
        },
        |frame| {
            frame.flex(1.0);
            frame.spacer(0.0);
            frame.icon(Icon::Radio, (size * 0.38).max(16.0));
            frame.flex(1.0);
            frame.spacer(0.0);
        },
    );
}

fn auxiliary_controls(app: &mut App, frame: &mut Frame, width: f32, language: Language) {
    frame.size_next(width, 52.0);
    frame.row_ex(
        &LayoutOpts {
            width,
            height: 52.0,
            gap: 4.0,
            pad: 4.0,
            cross: Align::Center,
            bg: Color::rgba(255, 255, 255, 7),
            radius: 14.0,
            ..LayoutOpts::default()
        },
        |frame| {
            if lyrics_button(app, frame) {
                app.view = if app.view == View::Lyrics {
                    View::Home
                } else {
                    View::Lyrics
                };
            }
            frame.flex(1.0);
            frame.spacer(0.0);
            volume_control(app, frame, language);
            frame.size_next(44.0, 40.0);
            if player_icon_button(frame, PlayerIcon::Queue, app.queue_panel_open()) {
                app.toggle_queue_panel();
            }
        },
    );
}

fn volume_control(app: &mut App, frame: &mut Frame, language: Language) {
    const OVERLAY_ID: &str = "player-volume-overlay";
    const OVERLAY_WIDTH: f32 = 58.0;
    const SLIDER_HEIGHT: f32 = 160.0;
    const VOLUME_STEP: f32 = 0.05;

    frame.size_next(44.0, 40.0);
    let clicked = player_icon_button(frame, volume_icon(app.volume), app.volume <= f32::EPSILON);
    let trigger = frame.response();
    let scrolled = frame.adjust_float_on_scroll(&mut app.volume, 0.0, 1.0, VOLUME_STEP);

    if clicked {
        app.toggle_mute();
    } else if scrolled {
        app.apply_volume_with_feedback();
    }
    if trigger.hovered || scrolled {
        frame.overlay_open(OVERLAY_ID);
    }

    let overlay_hovered = frame.overlay_hovered(OVERLAY_ID);
    let anchor = Rect {
        x: trigger.rect.x - ((OVERLAY_WIDTH - trigger.rect.w) * 0.5),
        y: trigger.rect.y,
        w: OVERLAY_WIDTH,
        h: trigger.rect.h,
    };
    let accent = PRESETS[app.preset % PRESETS.len()].accent;
    let base = theme(app.preset);
    let volume_theme = base
        .with_slider_track_color(Color::rgba(255, 255, 255, 46))
        .with_slider_fill_color(Color::rgba(accent[0], accent[1], accent[2], 255))
        .with_slider_knob_color(Color::rgba(252, 252, 253, 255));
    let mut slider_changed = false;
    let mut slider_pressed = false;
    frame.set_theme(volume_theme);
    frame.overlay(
        OVERLAY_ID,
        anchor,
        &OverlayOpts {
            gap: 3.0,
            pad: 8.0,
            cross: Align::Center,
            bg: Color::rgba(30, 32, 44, 250),
            border: Color::rgba(255, 255, 255, 18),
            border_width: 1.0,
            radius: 19.0,
            min_width: OVERLAY_WIDTH,
        },
        |frame| {
            frame.size_next(0.0, 24.0);
            frame.label_compact_sized(&format!("{:.0}", f64::from(app.volume) * 100.0), 14.0);
            frame.size_next(32.0, SLIDER_HEIGHT);
            slider_changed = frame.slider_vertical(
                text(language, Key::Volume),
                &mut app.volume,
                0.0,
                1.0,
                VOLUME_STEP,
            );
            slider_pressed = frame.response().pressed;
        },
    );
    frame.set_theme(base);

    if slider_changed {
        app.apply_volume_with_feedback();
    }
    if frame.overlay_is_open(OVERLAY_ID)
        && !trigger.hovered
        && !trigger.pressed
        && !overlay_hovered
        && !slider_pressed
    {
        frame.overlay_close(OVERLAY_ID);
    }
}

fn lyrics_button(app: &App, frame: &mut Frame) -> bool {
    let base = theme(app.preset);
    let accent = PRESETS[app.preset % PRESETS.len()].accent;
    let active = app.view == View::Lyrics;
    let tile = base
        .with_font_size(18.0)
        .with_accent(if active {
            Color::rgba(accent[0], accent[1], accent[2], 46)
        } else {
            Color::rgba(255, 255, 255, 8)
        })
        .with_active(Color::rgba(255, 255, 255, 22))
        .with_border_width(0.0)
        .with_corner_radius(12.0);
    frame.set_theme(tile);
    frame.size_next(44.0, 40.0);
    let clicked = frame.button("L##lyrics-control");
    frame.set_theme(base);
    clicked
}

fn playback_controls(app: &mut App, frame: &mut Frame, width: f32) {
    frame.size_next(width, PLAYER_HEIGHT - 20.0);
    frame.column_ex(
        &LayoutOpts {
            width,
            height: PLAYER_HEIGHT - 20.0,
            gap: 2.0,
            cross: Align::Stretch,
            ..LayoutOpts::default()
        },
        |frame| {
            frame.row_ex(
                &LayoutOpts {
                    height: 58.0,
                    gap: 7.0,
                    cross: Align::Center,
                    ..LayoutOpts::default()
                },
                |frame| {
                    frame.flex(1.0);
                    frame.spacer(0.0);
                    frame.size_next(42.0, 40.0);
                    if player_icon_button(
                        frame,
                        PlayerIcon::Shuffle,
                        app.playback_mode == PlaybackMode::Shuffle,
                    ) {
                        toggle_playback_mode(app, PlaybackMode::Shuffle);
                    }
                    frame.size_next(46.0, 42.0);
                    if player_icon_button(frame, PlayerIcon::Previous, false) {
                        app.previous();
                    }
                    frame.size_next(54.0, 46.0);
                    if player_icon_button(
                        frame,
                        if app.playback_state.is_playing() {
                            PlayerIcon::Pause
                        } else {
                            PlayerIcon::Play
                        },
                        true,
                    ) {
                        app.toggle_playback();
                    }
                    frame.size_next(46.0, 42.0);
                    if player_icon_button(frame, PlayerIcon::Next, false) {
                        app.next();
                    }
                    frame.size_next(42.0, 40.0);
                    if player_icon_button(
                        frame,
                        PlayerIcon::RepeatOne,
                        app.playback_mode == PlaybackMode::RepeatOne,
                    ) {
                        toggle_playback_mode(app, PlaybackMode::RepeatOne);
                    }
                    frame.flex(1.0);
                    frame.spacer(0.0);
                },
            );
            playback_timeline(app, frame, width);
        },
    );
}

fn playback_timeline(app: &mut App, frame: &mut Frame, width: f32) {
    frame.row_ex(
        &LayoutOpts {
            width,
            height: 24.0,
            gap: TIMELINE_GAP,
            cross: Align::Center,
            ..LayoutOpts::default()
        },
        |frame| {
            timeline_time(frame, &format_duration(app.position_ms), true);
            let timeline_width = (width - TIMELINE_TIME_WIDTH * 2.0 - TIMELINE_GAP * 2.0).max(1.0);
            frame.size_next(timeline_width, 18.0);
            let base = theme(app.preset);
            frame.set_theme(
                base.with_slider_track_thickness(TIMELINE_TRACK_THICKNESS)
                    .with_slider_knob_size(TIMELINE_KNOB_SIZE),
            );
            let changed = frame.slider("##timeline", &mut app.seek_ratio, 0.0, 1.0);
            frame.set_theme(base);
            if changed {
                app.commit_seek();
            }
            timeline_time(frame, &format_duration(app.duration_ms), false);
        },
    );
}

fn timeline_time(frame: &mut Frame, value: &str, align_end: bool) {
    frame.size_next(TIMELINE_TIME_WIDTH, 22.0);
    frame.row_ex(
        &LayoutOpts {
            width: TIMELINE_TIME_WIDTH,
            height: 22.0,
            cross: Align::Center,
            ..LayoutOpts::default()
        },
        |frame| {
            if align_end {
                frame.flex(1.0);
                frame.spacer(0.0);
            }
            frame.label_compact_sized(value, 9.5);
            if !align_end {
                frame.flex(1.0);
                frame.spacer(0.0);
            }
        },
    );
}

fn toggle_playback_mode(app: &mut App, mode: PlaybackMode) {
    app.set_playback_mode(if app.playback_mode == mode {
        PlaybackMode::Sequential
    } else {
        mode
    });
}

fn playback_mode_toast(app: &App, frame: &mut Frame, width: f32, height: f32, language: Language) {
    let Some(toast) = app.playback_mode_toast() else {
        return;
    };
    let toast_width = 164.0;
    let opacity = toast.opacity.clamp(0.0, 1.0);
    let alpha = metric_alpha(opacity * 255.0);
    let accent = PRESETS[app.preset % PRESETS.len()].accent;
    let base = theme(app.preset);
    frame.set_theme(base.with_fg(base.fg().with_alpha(alpha)));
    frame.layer(
        "playback-mode-toast",
        Rect {
            x: ((width - toast_width) * 0.5).max(8.0),
            y: (height - ROOT_PAD - PLAYER_HEIGHT - 48.0 + toast.offset_y).max(8.0),
            w: toast_width,
            h: 38.0,
        },
        &OverlayOpts {
            pad: 10.0,
            cross: Align::Center,
            bg: Color::rgba(10, 13, 18, metric_alpha(opacity * 236.0)),
            border: Color::rgba(
                accent[0],
                accent[1],
                accent[2],
                metric_alpha(opacity * 92.0),
            ),
            border_width: 1.0,
            radius: 14.0,
            min_width: toast_width,
            ..OverlayOpts::default()
        },
        |frame| {
            frame.label_compact_sized(playback_mode_label(language, toast.mode), 12.0);
        },
    );
    frame.set_theme(base);
}

#[derive(Debug, Clone, Copy)]
enum PlayerIcon {
    RepeatOne,
    Shuffle,
    Previous,
    Play,
    Pause,
    Next,
    VolumeMuted,
    VolumeLow,
    VolumeHigh,
    Queue,
}

impl PlayerIcon {
    fn icon(self) -> Icon {
        match self {
            Self::RepeatOne => Icon::Repeat,
            Self::Shuffle => Icon::Shuffle,
            Self::Previous => Icon::SkipBack,
            Self::Play => Icon::Play,
            Self::Pause => Icon::Pause,
            Self::Next => Icon::SkipForward,
            Self::VolumeMuted => Icon::VolumeMuted,
            Self::VolumeLow => Icon::VolumeLow,
            Self::VolumeHigh => Icon::VolumeHigh,
            Self::Queue => Icon::Menu,
        }
    }
}

fn playback_mode_label(language: Language, mode: PlaybackMode) -> &'static str {
    match mode {
        PlaybackMode::Sequential => text(language, Key::Sequential),
        PlaybackMode::RepeatOne => text(language, Key::RepeatOne),
        PlaybackMode::Shuffle => text(language, Key::Shuffle),
    }
}

fn volume_icon(volume: f32) -> PlayerIcon {
    if volume <= 0.001 {
        PlayerIcon::VolumeMuted
    } else if volume < 0.5 {
        PlayerIcon::VolumeLow
    } else {
        PlayerIcon::VolumeHigh
    }
}

fn player_icon_button(frame: &mut Frame, icon: PlayerIcon, active: bool) -> bool {
    let button = PlayerControlButton::new(icon.icon()).active(active);
    let button = if matches!(icon, PlayerIcon::RepeatOne) {
        button.badge("1")
    } else {
        button
    };
    player_control_button(frame, button)
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

    #[test]
    fn visual_theme_uses_each_preset_palette() {
        for (index, preset) in PRESETS.iter().enumerate() {
            let themed = theme(index);
            assert_eq!(
                themed.accent(),
                Color::rgba(preset.accent[0], preset.accent[1], preset.accent[2], 255)
            );
            assert_eq!(
                themed.active(),
                Color::rgba(preset.accent[0], preset.accent[1], preset.accent[2], 38)
            );
        }
    }

    #[test]
    fn content_scrims_preserve_glow_by_information_density() {
        assert_eq!(content_surface_alpha(View::Visuals), 92);
        assert_eq!(content_surface_alpha(View::Home), 128);
        assert_eq!(content_surface_alpha(View::Library), 148);
        assert!(content_surface_alpha(View::Home) < content_surface_alpha(View::Library));
    }

    #[test]
    fn visual_inspector_never_squeezes_the_side_stage_below_its_minimum() {
        let threshold = VISUAL_MIN_SIDE_STAGE_WIDTH + VISUAL_INSPECTOR_WIDTH + VISUAL_STAGE_GAP;

        assert!(!uses_side_visual_layout(threshold - 0.5));
        assert!(uses_side_visual_layout(threshold));
        assert!(
            (side_visual_stage_width(threshold) - VISUAL_MIN_SIDE_STAGE_WIDTH).abs() < f32::EPSILON
        );
        assert!((stacked_visual_stage_height(560.0) - 212.8).abs() < 0.01);
        assert!((stacked_visual_stage_height(1_000.0) - 260.0).abs() < f32::EPSILON);
        assert!(uses_compact_visual_stage(stacked_visual_stage_height(
            560.0
        )));
        assert!(uses_compact_visual_stage(VISUAL_COMPACT_STAGE_HEIGHT - 0.5));
        assert!(!uses_compact_visual_stage(VISUAL_COMPACT_STAGE_HEIGHT));
    }
}
