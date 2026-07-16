//! Small, typed translation catalog for Wavora.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LanguagePreference {
    #[default]
    System,
    English,
    SimplifiedChinese,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    SimplifiedChinese,
}

impl LanguagePreference {
    #[must_use]
    pub fn resolve(self) -> Language {
        match self {
            Self::System => system_language(),
            Self::English => Language::English,
            Self::SimplifiedChinese => Language::SimplifiedChinese,
        }
    }
}

#[must_use]
pub fn system_language() -> Language {
    ["LC_ALL", "LC_MESSAGES", "LANGUAGE", "LANG"]
        .into_iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.trim().is_empty())
        .map_or(Language::English, |locale| language_from_locale(&locale))
}

#[must_use]
pub fn language_from_locale(locale: &str) -> Language {
    // GNU `LANGUAGE` may contain an ordered colon-separated fallback list.
    let primary = locale.split(':').next().unwrap_or(locale);
    let normalized = primary.trim().to_ascii_lowercase().replace('_', "-");
    if normalized == "zh" || normalized.starts_with("zh-") {
        Language::SimplifiedChinese
    } else {
        Language::English
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    AppSubtitle,
    YourSpace,
    Now,
    Library,
    Favorites,
    Playlists,
    Lyrics,
    VisualStage,
    Collection,
    LocalTracks,
    FavoriteTracks,
    Scanning,
    Search,
    AddFile,
    AddFolder,
    Settings,
    ImmersiveListening,
    SoundInMotion,
    EmptyHomeDescription,
    AddMusicFolder,
    LibraryCard,
    VisualCard,
    EngineCard,
    LocalArchive,
    SystemDecode,
    LocalLibrary,
    Tracks,
    EmptyLibrary,
    Title,
    Artist,
    Album,
    Duration,
    VisualDescription,
    VisualFootnote,
    VisualPreview,
    VisualControls,
    VisualPresets,
    Composition,
    ResponseTuning,
    SceneLayers,
    Material,
    Placement,
    Appearance,
    MotionAndAudio,
    Live,
    Listening,
    WaitingForAudio,
    Loudness,
    Pitch,
    Centroid,
    Onset,
    Intensity,
    Motion,
    Depth,
    Glow,
    Atmosphere,
    AtmosphereEnabled,
    CompositionVisible,
    MaterialField,
    NoMaterial,
    Watercolor,
    Caustics,
    TextureScale,
    FieldMotion,
    NewVariation,
    LightSources,
    LightSource,
    AddLight,
    RemoveLight,
    Palette,
    FollowPreset,
    CustomColor,
    SourceShape,
    Circle,
    Oval,
    Beam,
    AspectRatio,
    Rotation,
    Falloff,
    Diffuse,
    Focused,
    Halo,
    Horizontal,
    Vertical,
    Radius,
    Strength,
    Hue,
    Saturation,
    Drift,
    AudioResponse,
    NoResponse,
    Energy,
    Bass,
    Midrange,
    Treble,
    ScaleResponse,
    StrengthResponse,
    AtmosphereHint,
    SettingsDescription,
    DefaultVolume,
    Language,
    FollowSystem,
    English,
    SimplifiedChinese,
    ConfigFile,
    StateFile,
    FavoritesFile,
    CatalogFile,
    MusicFolders,
    NoMusicFolder,
    SupportedFormats,
    UpNext,
    Queue,
    EmptyQueue,
    NothingPlaying,
    AddLocalTrack,
    Volume,
    PlaybackMode,
    Sequential,
    RepeatOne,
    Shuffle,
    NoLyrics,
    LyricsSidecarHint,
    ReloadLyrics,
    LyricsLoadFailed,
    RestorePersistence,
    PlaybackFailed,
    SavePersistenceFailed,
    UnsupportedFile,
    InvalidFilePath,
    InvalidFolder,
    InvalidFolderPath,
    AddedTracks,
    ScanSummary,
    Favorited,
    Unfavorited,
    NewPlaylist,
    CreatePlaylist,
    ListView,
    CoverView,
    AddCurrentTrack,
    RemoveFromPlaylist,
    MoveUp,
    MoveDown,
    DeletePlaylist,
    ConfirmDeletePlaylist,
    NoPlaylists,
    MissingTrack,
}

#[must_use]
pub const fn text(language: Language, key: Key) -> &'static str {
    match language {
        Language::English => english(key),
        Language::SimplifiedChinese => chinese(key),
    }
}

#[allow(clippy::too_many_lines)]
const fn english(key: Key) -> &'static str {
    match key {
        Key::AppSubtitle => "LOCAL AUDIO SPACE",
        Key::YourSpace => "YOUR SPACE",
        Key::Now => "Now playing",
        Key::Library => "Library",
        Key::Favorites => "Favorites",
        Key::Playlists => "Playlists",
        Key::Lyrics => "Lyrics",
        Key::VisualStage => "Visual stage",
        Key::Collection => "COLLECTION",
        Key::LocalTracks => "local tracks",
        Key::FavoriteTracks => "favorites",
        Key::Scanning => "Reading media metadata…",
        Key::Search => "Search music",
        Key::AddFile => "Add music file",
        Key::AddFolder => "Add music folder",
        Key::Settings => "Settings",
        Key::ImmersiveListening => "IMMERSIVE LOCAL LISTENING",
        Key::SoundInMotion => "Your sound, in motion.",
        Key::EmptyHomeDescription => {
            "Add a local music folder to begin building your private listening space."
        }
        Key::AddMusicFolder => "+ Add music folder",
        Key::LibraryCard => "LIBRARY",
        Key::VisualCard => "VISUAL",
        Key::EngineCard => "ENGINE",
        Key::LocalArchive => "Your local sound archive",
        Key::SystemDecode => "Built-in decoding · native output",
        Key::LocalLibrary => "Local library",
        Key::Tracks => "TRACKS",
        Key::EmptyLibrary => "No playable tracks yet. Add a music folder to get started.",
        Key::Title => "Title",
        Key::Artist => "Artist",
        Key::Album => "Album",
        Key::Duration => "Time",
        Key::VisualDescription => {
            "Six independent compositions turn frequency, pitch, loudness and transients into motion."
        }
        Key::VisualFootnote => {
            "Driven by 32-band PCM analysis, bass/mid/treble, dBFS loudness, pitch, centroid and onset."
        }
        Key::VisualPreview => "AUDIO-REACTIVE STAGE",
        Key::VisualControls => "Visual controls",
        Key::VisualPresets => "Compositions",
        Key::Composition => "Composition",
        Key::ResponseTuning => "Composition response",
        Key::SceneLayers => "Scene layers",
        Key::Material => "Material",
        Key::Placement => "Placement and size",
        Key::Appearance => "Appearance",
        Key::MotionAndAudio => "Motion and audio",
        Key::Live => "LIVE",
        Key::Listening => "Listening to decoded PCM",
        Key::WaitingForAudio => "Play a track to wake the stage",
        Key::Loudness => "Loudness",
        Key::Pitch => "Pitch",
        Key::Centroid => "Centroid",
        Key::Onset => "Onset",
        Key::Intensity => "Response",
        Key::Motion => "Motion",
        Key::Depth => "Depth",
        Key::Glow => "Glow",
        Key::Atmosphere => "Atmosphere",
        Key::AtmosphereEnabled => "Atmosphere enabled",
        Key::CompositionVisible => "Composition visible",
        Key::MaterialField => "Material field",
        Key::NoMaterial => "None",
        Key::Watercolor => "Watercolour diffusion",
        Key::Caustics => "Water caustics",
        Key::TextureScale => "Material scale",
        Key::FieldMotion => "Material drift",
        Key::NewVariation => "New material variation",
        Key::LightSources => "Light sources",
        Key::LightSource => "Light",
        Key::AddLight => "+ Add light",
        Key::RemoveLight => "Remove light",
        Key::Palette => "Colour source",
        Key::FollowPreset => "Follow composition",
        Key::CustomColor => "Custom colour",
        Key::SourceShape => "Source shape",
        Key::Circle => "Circle",
        Key::Oval => "Oval area light",
        Key::Beam => "Directional beam",
        Key::AspectRatio => "Elongation",
        Key::Rotation => "Direction",
        Key::Falloff => "Falloff",
        Key::Diffuse => "Diffuse",
        Key::Focused => "Focused",
        Key::Halo => "Halo",
        Key::Horizontal => "Horizontal position",
        Key::Vertical => "Vertical position",
        Key::Radius => "Radius",
        Key::Strength => "Strength",
        Key::Hue => "Hue",
        Key::Saturation => "Saturation",
        Key::Drift => "Independent drift",
        Key::AudioResponse => "Audio response",
        Key::NoResponse => "Off",
        Key::Energy => "Energy",
        Key::Bass => "Bass",
        Key::Midrange => "Midrange",
        Key::Treble => "Treble",
        Key::ScaleResponse => "Audio size",
        Key::StrengthResponse => "Audio brightness",
        Key::AtmosphereHint => {
            "Positions may extend beyond the window; only the light tail remains visible."
        }
        Key::SettingsDescription => "Playback and visual preferences are stored locally.",
        Key::DefaultVolume => "Default volume",
        Key::Language => "Language",
        Key::FollowSystem => "Follow system",
        Key::English => "English",
        Key::SimplifiedChinese => "简体中文",
        Key::ConfigFile => "Configuration",
        Key::StateFile => "Session state",
        Key::FavoritesFile => "Favorites data",
        Key::CatalogFile => "Music catalog",
        Key::MusicFolders => "Music folders",
        Key::NoMusicFolder => "None added",
        Key::SupportedFormats => "Built-in: FLAC · MP3 · M4A/AAC · Ogg Vorbis · WAV",
        Key::UpNext => "Up next",
        Key::Queue => "QUEUE",
        Key::EmptyQueue => "The queue is empty",
        Key::NothingPlaying => "Nothing playing",
        Key::AddLocalTrack => "Add a local track to begin",
        Key::Volume => "VOLUME",
        Key::PlaybackMode => "PLAYBACK MODE",
        Key::Sequential => "Sequential",
        Key::RepeatOne => "Repeat one",
        Key::Shuffle => "Shuffle",
        Key::NoLyrics => "No synchronized lyrics for this track",
        Key::LyricsSidecarHint => "Add a .wlyric.json sidecar next to the audio file.",
        Key::ReloadLyrics => "Reload lyrics",
        Key::LyricsLoadFailed => "Could not load lyrics",
        Key::RestorePersistence => "Stored data recovered; invalid file(s) saved at",
        Key::PlaybackFailed => "Playback failed",
        Key::SavePersistenceFailed => "Could not save local data",
        Key::UnsupportedFile => "That file format is not supported",
        Key::InvalidFilePath => "The selected file path could not be read",
        Key::InvalidFolder => "The selected location is not a folder",
        Key::InvalidFolderPath => "The selected folder path could not be read",
        Key::AddedTracks => "tracks added",
        Key::ScanSummary => "files could not be decoded and were skipped",
        Key::Favorited => "Added to favorites",
        Key::Unfavorited => "Removed from favorites",
        Key::NewPlaylist => "New playlist name",
        Key::CreatePlaylist => "Create",
        Key::ListView => "List",
        Key::CoverView => "Covers",
        Key::AddCurrentTrack => "Add playing track",
        Key::RemoveFromPlaylist => "Remove",
        Key::MoveUp => "Move up",
        Key::MoveDown => "Move down",
        Key::DeletePlaylist => "Delete playlist",
        Key::ConfirmDeletePlaylist => "Select Delete playlist again to confirm",
        Key::NoPlaylists => "Create a playlist to organize your local music.",
        Key::MissingTrack => "Missing file",
    }
}

#[allow(clippy::too_many_lines)]
const fn chinese(key: Key) -> &'static str {
    match key {
        Key::AppSubtitle => "本地声场",
        Key::YourSpace => "你的空间",
        Key::Now => "此刻",
        Key::Library | Key::LibraryCard => "音乐库",
        Key::Favorites => "我喜欢",
        Key::Playlists => "歌单",
        Key::Lyrics => "歌词",
        Key::VisualStage => "视觉舞台",
        Key::Collection => "收藏概览",
        Key::LocalTracks => "首本地曲目",
        Key::FavoriteTracks => "首已收藏",
        Key::Scanning => "正在读取媒体信息…",
        Key::Search => "搜索音乐",
        Key::AddFile => "添加音乐文件",
        Key::AddFolder => "添加音乐文件夹",
        Key::Settings => "设置",
        Key::ImmersiveListening => "沉浸式本地聆听",
        Key::SoundInMotion => "让声音，流动起来。",
        Key::EmptyHomeDescription => "添加本地音乐文件夹，开始构建只属于你的聆听空间。",
        Key::AddMusicFolder => "＋ 添加音乐文件夹",
        Key::VisualCard => "视觉",
        Key::EngineCard => "引擎",
        Key::LocalArchive => "你的本地声音档案",
        Key::SystemDecode => "内置解码 · 原生输出",
        Key::LocalLibrary => "本地音乐库",
        Key::Tracks => "首曲目",
        Key::EmptyLibrary => "还没有可播放的曲目。添加音乐文件夹即可开始。",
        Key::Title => "标题",
        Key::Artist => "艺人",
        Key::Album => "专辑",
        Key::Duration => "时长",
        Key::VisualDescription => "六种独立构图，把频率、音高、响度和瞬态变成不同的空间运动。",
        Key::VisualFootnote => {
            "由 32 段 PCM 频谱、低中高频、dBFS 响度、音高、频谱质心与瞬态共同驱动。"
        }
        Key::VisualPreview => "音频响应舞台",
        Key::VisualControls => "视觉控制",
        Key::VisualPresets => "构图预设",
        Key::Composition => "构图",
        Key::ResponseTuning => "构图响应",
        Key::SceneLayers => "场景图层",
        Key::Material => "材质",
        Key::Placement => "位置与尺寸",
        Key::Appearance => "外观",
        Key::MotionAndAudio => "运动与音频",
        Key::Live => "实时",
        Key::Listening => "正在解析 PCM 音频",
        Key::WaitingForAudio => "播放音乐，唤醒舞台",
        Key::Loudness => "响度",
        Key::Pitch => "音高",
        Key::Centroid => "频谱质心",
        Key::Onset => "瞬态",
        Key::Intensity => "响应强度",
        Key::Motion => "运动速率",
        Key::Depth => "空间纵深",
        Key::Glow => "辉光",
        Key::Atmosphere => "氛围层",
        Key::AtmosphereEnabled => "启用氛围层",
        Key::CompositionVisible => "显示主体构图",
        Key::MaterialField => "氛围材质",
        Key::NoMaterial => "无",
        Key::Watercolor => "水彩浸染",
        Key::Caustics => "水面焦散",
        Key::TextureScale => "材质尺度",
        Key::FieldMotion => "材质漂移",
        Key::NewVariation => "生成新的材质变化",
        Key::LightSources | Key::LightSource => "光源",
        Key::AddLight => "＋ 添加光源",
        Key::RemoveLight => "移除光源",
        Key::Palette => "颜色来源",
        Key::FollowPreset => "跟随构图配色",
        Key::CustomColor => "自定义颜色",
        Key::SourceShape => "光源形状",
        Key::Circle => "圆形",
        Key::Oval => "椭圆面积光",
        Key::Beam => "方向光束",
        Key::AspectRatio => "拉伸比例",
        Key::Rotation => "光源方向",
        Key::Falloff => "晕开样式",
        Key::Diffuse => "扩散",
        Key::Focused => "聚焦",
        Key::Halo => "光环",
        Key::Horizontal => "水平位置",
        Key::Vertical => "垂直位置",
        Key::Radius => "光晕半径",
        Key::Strength => "光源强度",
        Key::Hue => "色相",
        Key::Saturation => "饱和度",
        Key::Drift => "独立漂移",
        Key::AudioResponse => "音频响应",
        Key::NoResponse => "关闭",
        Key::Energy => "整体能量",
        Key::Bass => "低频",
        Key::Midrange => "中频",
        Key::Treble => "高频",
        Key::ScaleResponse => "音频尺寸响应",
        Key::StrengthResponse => "音频亮度响应",
        Key::AtmosphereHint => "位置可以超出窗口边界，只让光晕尾部进入画面。",
        Key::SettingsDescription => "播放与视觉偏好只保存在本机。",
        Key::DefaultVolume => "默认音量",
        Key::Language => "语言",
        Key::FollowSystem => "跟随系统",
        Key::English => "English",
        Key::SimplifiedChinese => "简体中文",
        Key::ConfigFile => "配置文件",
        Key::StateFile => "会话状态",
        Key::FavoritesFile => "收藏数据",
        Key::CatalogFile => "音乐目录数据库",
        Key::MusicFolders => "音乐文件夹",
        Key::NoMusicFolder => "尚未添加",
        Key::SupportedFormats => "内置支持：FLAC · MP3 · M4A/AAC · Ogg Vorbis · WAV",
        Key::UpNext => "接下来播放",
        Key::Queue => "队列",
        Key::EmptyQueue => "队列为空",
        Key::NothingPlaying => "没有正在播放的音乐",
        Key::AddLocalTrack => "添加本地曲目开始",
        Key::Volume => "音量",
        Key::PlaybackMode => "播放模式",
        Key::Sequential => "顺序播放",
        Key::RepeatOne => "单曲重复",
        Key::Shuffle => "随机播放",
        Key::NoLyrics => "当前曲目没有同步歌词",
        Key::LyricsSidecarHint => "在音频文件旁添加 .wlyric.json 歌词文件。",
        Key::ReloadLyrics => "重新载入歌词",
        Key::LyricsLoadFailed => "歌词载入失败",
        Key::RestorePersistence => "本地数据已恢复，损坏文件保存在",
        Key::PlaybackFailed => "播放失败",
        Key::SavePersistenceFailed => "保存本地数据失败",
        Key::UnsupportedFile => "暂不支持所选文件格式",
        Key::InvalidFilePath => "无法读取所选文件路径",
        Key::InvalidFolder => "所选位置不是文件夹",
        Key::InvalidFolderPath => "无法读取所选文件夹路径",
        Key::AddedTracks => "首曲目已加入",
        Key::ScanSummary => "个文件无法解码，已跳过",
        Key::Favorited => "已收藏",
        Key::Unfavorited => "已取消收藏",
        Key::NewPlaylist => "新歌单名称",
        Key::CreatePlaylist => "创建",
        Key::ListView => "列表",
        Key::CoverView => "封面",
        Key::AddCurrentTrack => "加入正在播放的曲目",
        Key::RemoveFromPlaylist => "移除",
        Key::MoveUp => "上移",
        Key::MoveDown => "下移",
        Key::DeletePlaylist => "删除歌单",
        Key::ConfirmDeletePlaylist => "再次选择“删除歌单”以确认",
        Key::NoPlaylists => "创建一个歌单，整理你的本地音乐。",
        Key::MissingTrack => "文件已丢失",
    }
}

/// Localized display copy for a built-in visual composition.
///
/// Visual rendering deliberately stays language-neutral; UI copy lives here
/// so switching languages never leaves a half-translated preset row behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisualPresetText {
    pub name: &'static str,
    pub subtitle: &'static str,
    pub response: &'static str,
}

#[must_use]
pub const fn visual_preset_text(language: Language, index: usize) -> VisualPresetText {
    match language {
        Language::English => match index {
            0 => VisualPresetText {
                name: "Particle Veil",
                subtitle: "A folded particle fabric",
                response: "Pitch · mids · spectral centroid",
            },
            1 => VisualPresetText {
                name: "Pulse Tunnel",
                subtitle: "A low-frequency depth tunnel",
                response: "Bass · transients · loudness",
            },
            2 => VisualPresetText {
                name: "Orbital Core",
                subtitle: "Frequency in orbit",
                response: "Dominant frequency · treble · energy",
            },
            3 => VisualPresetText {
                name: "Spectral Void",
                subtitle: "A transient eclipse",
                response: "Spectrum · peak · transients",
            },
            4 => VisualPresetText {
                name: "Vinyl Halo",
                subtitle: "Loudness etched into grooves",
                response: "Loudness · bass · spectrum",
            },
            _ => VisualPresetText {
                name: "Star River",
                subtitle: "A three-band flow field",
                response: "Bass · mids · treble",
            },
        },
        Language::SimplifiedChinese => match index {
            0 => VisualPresetText {
                name: "粒子帷幕",
                subtitle: "折叠的粒子织面",
                response: "音高 · 中频 · 频谱质心",
            },
            1 => VisualPresetText {
                name: "脉冲隧道",
                subtitle: "低频塑造纵深",
                response: "低频 · 瞬态 · 响度",
            },
            2 => VisualPresetText {
                name: "轨道核心",
                subtitle: "让频率进入轨道",
                response: "主频 · 高频 · 能量",
            },
            3 => VisualPresetText {
                name: "频谱虚空",
                subtitle: "瞬态掠过日蚀",
                response: "频谱 · 峰值 · 瞬态",
            },
            4 => VisualPresetText {
                name: "黑胶光环",
                subtitle: "把响度刻进声槽",
                response: "响度 · 低频 · 频谱",
            },
            _ => VisualPresetText {
                name: "星河",
                subtitle: "三频交织的流场",
                response: "低频 · 中频 · 高频",
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_chinese_locale_variants() {
        assert_eq!(
            language_from_locale("zh_CN.UTF-8"),
            Language::SimplifiedChinese
        );
        assert_eq!(language_from_locale("zh-Hans"), Language::SimplifiedChinese);
        assert_eq!(
            language_from_locale("zh_CN:en_US"),
            Language::SimplifiedChinese
        );
        assert_eq!(language_from_locale("en_US.UTF-8"), Language::English);
    }

    #[test]
    fn catalog_has_both_languages() {
        assert_eq!(text(Language::English, Key::Library), "Library");
        assert_eq!(text(Language::SimplifiedChinese, Key::Library), "音乐库");
    }

    #[test]
    fn visual_presets_are_fully_localized() {
        for index in 0..6 {
            let english = visual_preset_text(Language::English, index);
            assert!(
                !english
                    .name
                    .chars()
                    .chain(english.subtitle.chars())
                    .chain(english.response.chars())
                    .any(|character| ('\u{4e00}'..='\u{9fff}').contains(&character))
            );
            let chinese = visual_preset_text(Language::SimplifiedChinese, index);
            assert!(
                chinese
                    .name
                    .chars()
                    .chain(chinese.subtitle.chars())
                    .chain(chinese.response.chars())
                    .any(|character| ('\u{4e00}'..='\u{9fff}').contains(&character))
            );
        }
    }
}
