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
    SettingsDescription,
    DefaultVolume,
    Language,
    FollowSystem,
    English,
    SimplifiedChinese,
    ConfigFile,
    MusicFolders,
    NoMusicFolder,
    SupportedFormats,
    UpNext,
    Queue,
    EmptyQueue,
    NothingPlaying,
    AddLocalTrack,
    Volume,
    RestoreConfig,
    PlaybackFailed,
    SaveSettingsFailed,
    UnsupportedFile,
    InvalidFilePath,
    InvalidFolder,
    InvalidFolderPath,
    AddedTracks,
    ScanSummary,
    Favorited,
    Unfavorited,
}

#[must_use]
pub const fn text(language: Language, key: Key) -> &'static str {
    match language {
        Language::English => english(key),
        Language::SimplifiedChinese => chinese(key),
    }
}

const fn english(key: Key) -> &'static str {
    match key {
        Key::AppSubtitle => "LOCAL AUDIO SPACE",
        Key::YourSpace => "YOUR SPACE",
        Key::Now => "Now playing",
        Key::Library => "Library",
        Key::Favorites => "Favorites",
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
        Key::SettingsDescription => "Playback and visual preferences are stored locally.",
        Key::DefaultVolume => "Default volume",
        Key::Language => "Language",
        Key::FollowSystem => "Follow system",
        Key::English => "English",
        Key::SimplifiedChinese => "简体中文",
        Key::ConfigFile => "Configuration",
        Key::MusicFolders => "Music folders",
        Key::NoMusicFolder => "None added",
        Key::SupportedFormats => "Built-in: FLAC · MP3 · M4A/AAC · Ogg Vorbis · WAV",
        Key::UpNext => "Up next",
        Key::Queue => "QUEUE",
        Key::EmptyQueue => "The queue is empty",
        Key::NothingPlaying => "Nothing playing",
        Key::AddLocalTrack => "Add a local track to begin",
        Key::Volume => "VOLUME",
        Key::RestoreConfig => "Configuration recovered; the invalid file was saved at",
        Key::PlaybackFailed => "Playback failed",
        Key::SaveSettingsFailed => "Could not save settings",
        Key::UnsupportedFile => "That file format is not supported",
        Key::InvalidFilePath => "The selected file path could not be read",
        Key::InvalidFolder => "The selected location is not a folder",
        Key::InvalidFolderPath => "The selected folder path could not be read",
        Key::AddedTracks => "tracks added",
        Key::ScanSummary => "files could not be decoded and were skipped",
        Key::Favorited => "Added to favorites",
        Key::Unfavorited => "Removed from favorites",
    }
}

const fn chinese(key: Key) -> &'static str {
    match key {
        Key::AppSubtitle => "本地声场",
        Key::YourSpace => "你的空间",
        Key::Now => "此刻",
        Key::Library | Key::LibraryCard => "音乐库",
        Key::Favorites => "我喜欢",
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
        Key::SettingsDescription => "播放与视觉偏好只保存在本机。",
        Key::DefaultVolume => "默认音量",
        Key::Language => "语言",
        Key::FollowSystem => "跟随系统",
        Key::English => "English",
        Key::SimplifiedChinese => "简体中文",
        Key::ConfigFile => "配置文件",
        Key::MusicFolders => "音乐文件夹",
        Key::NoMusicFolder => "尚未添加",
        Key::SupportedFormats => "内置支持：FLAC · MP3 · M4A/AAC · Ogg Vorbis · WAV",
        Key::UpNext => "接下来播放",
        Key::Queue => "队列",
        Key::EmptyQueue => "队列为空",
        Key::NothingPlaying => "没有正在播放的音乐",
        Key::AddLocalTrack => "添加本地曲目开始",
        Key::Volume => "音量",
        Key::RestoreConfig => "配置已恢复，损坏文件保存在",
        Key::PlaybackFailed => "播放失败",
        Key::SaveSettingsFailed => "保存设置失败",
        Key::UnsupportedFile => "暂不支持所选文件格式",
        Key::InvalidFilePath => "无法读取所选文件路径",
        Key::InvalidFolder => "所选位置不是文件夹",
        Key::InvalidFolderPath => "无法读取所选文件夹路径",
        Key::AddedTracks => "首曲目已加入",
        Key::ScanSummary => "个文件无法解码，已跳过",
        Key::Favorited => "已收藏",
        Key::Unfavorited => "已取消收藏",
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
