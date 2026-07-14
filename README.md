# Wavora

Wavora 是一个本地、沉浸式、多媒体音乐播放器。它使用 Rust 构建业务层，以
[Optics](../optics) 的 Iris / Lens / Flux 图形栈呈现原生 Wayland + Vulkan UI。
Rodio / Symphonia 在应用内完成音频解码，解码后的 PCM 再交给 GStreamer 与本机
PipeWire、PulseAudio 或 ALSA 输出，因此播放常见格式不依赖 GStreamer 编解码插件。

当前产品方向：

- 本地优先，不依赖在线音乐账号或私有音源接口
- 暗色、克制、具有空间层次的音乐舞台
- 可切换的粒子视觉气质，以实时 PCM 能量和 16 段频谱驱动画面
- 播放、媒体扫描和渲染分离，避免阻塞 UI 帧
- 跟随系统语言，也可在设置中固定为 English 或简体中文
- 虚拟化曲目表格、固定表头、可见滚动条和符合 Wayland 习惯的滚动方向
- 配置原子写入，启动恢复最近曲目、收藏、音量和视觉预设；损坏配置会备份后自动恢复

内置解码格式：FLAC、MP3、M4A/AAC、Ogg Vorbis 和 WAV。扫描阶段会实际打开文件并
读取时长；不能解码的文件会被跳过并给出汇总提示。

## 运行

先构建相邻的 Optics：

```bash
meson setup ../optics/build ../optics -Dexamples=true
meson compile -C ../optics/build
```

然后运行 Wavora：

```bash
./run.sh
```

也可以把音乐文件或目录传给它：

```bash
./run.sh ~/Music
./run.sh ~/Music/example.flac
```

本地安装（默认安装到 `~/.local`）会同时放置桌面启动器、图标、AppStream 元数据和
Optics 运行库：

```bash
./packaging/install.sh
```

可通过 `PREFIX=/custom/prefix ./packaging/install.sh` 修改目标前缀。GStreamer 基础库、
本机音频输出插件以及 Vulkan/Wayland 驱动仍由系统提供。

## 验证

```bash
cargo fmt --all --check
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --release --locked
cargo audit --no-fetch
```

运行时依赖包括 Vulkan 1.3、Wayland、GStreamer 1.20+、`appsrc`、`audioconvert`、
`audioresample`、`volume`，以及 PipeWire、PulseAudio、自动音频或 ALSA 输出插件之一。
不要求安装 GStreamer 的 FLAC/MP3/AAC 解码插件。

工作区按职责分为 `wavora-core`（领域模型）、`wavora-i18n`（语言解析与文案）、
`wavora-media`（扫描、解码与输出）和根应用（状态、持久化、Optics UI/视觉）。
架构与设计约束见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) 和
[docs/DESIGN.md](docs/DESIGN.md)。
