# Wavora Architecture

Wavora 沿用 Termus 已验证过的边界思想，但以 Rust channel 和 Optics 桌面栈实现。
代码是 Cargo workspace，各 crate 只暴露相邻层真正需要的 API。

```text
Iris main thread
  ├─ Lens UI / input / app state
  ├─ wavora-visuals ──> Flux paint callback (visual state snapshot)
  ├─ commands ──> audio worker ──> Rodio/Symphonia decoder
  │                              ├─ wavora-audio-analysis
  │                              │    └─ PCM ──> 32 bands + pitch/loudness/onset
  │                              └─ GStreamer appsrc ──> native sound server
  └─ commands ──> library worker ──> filesystem + decoder validation
```

## Workspace 与依赖方向

```text
wavora (binary + app)
  ├─ wavora-core
  ├─ wavora-i18n
  ├─ wavora-media ──> wavora-core
  │                └─ wavora-audio-analysis
  ├─ wavora-visuals ──> wavora-audio-analysis
  │                  └─ Optics Iris / Flux
  └─ Optics Iris / Lens / Flux

wavora-core    Track、PlaybackState 与纯格式化逻辑
wavora-audio-analysis  与播放后端无关的 PCM 特征帧：频谱、音高、响度、三频、瞬态
wavora-i18n    系统 locale 解析、语言偏好与类型化文案表
wavora-media   文件 URI、异步扫描、内置解码、分析调度与原生输出
wavora-visuals 六套独立构图、音频响应包络、预设转场与 Flux 绘制
wavora         应用状态、配置持久化与 UI 编排
```

- UI 不拥有解码器或扫描器。
- 音频线程不触碰 Lens / Flux。
- Flux paint callback 只读一份轻量视觉快照，不锁住 App 状态；视觉 crate 不依赖媒体层。
- 音频分析 crate 不依赖解码器、GStreamer 或 UI；seek 时清空瞬态历史，避免伪鼓点。
- 文件扫描使用可取消的流式遍历，并在工作线程中验证解码能力、读取真实时长。
- 配置使用同目录临时文件 + rename 原子替换。
- 播放器由 Symphonia/Rodio 解码为 `f32` PCM；GStreamer 只负责格式转换、重采样、
  音量和本机输出，避免系统缺少 GStreamer codec 插件时常见格式无法播放。
- seek 通过 GStreamer 时间轴回调到解码器；EOS 同时使用总线消息和末尾位置保护。
- UI 字符串通过 `wavora-i18n::Key` 访问；默认偏好为 `System`，启动时解析系统 locale。

## 表格与滚动

曲目表格由 Lens 安全 Rust API 提供并按可视行虚拟化。表头与表体使用嵌套裁剪，单元格
单独裁剪，确保长标题或艺人不会覆盖相邻列；滚动条始终可发现。Wayland 的物理滚动轴
在 Iris 平台边界转换为 Lens 的逻辑约定，因此“向下滚”在所有 Lens 滚动控件中一致。

## Optics 边界

属于通用图形/UI 能力的缺口应在 `../optics` 修复。本次在 Optics 中补齐了安全 Rust
table API、虚拟化回调、单元格/表体裁剪、嵌套裁剪回放以及 Wayland 滚动轴转换；这些
能力不包含任何 Wavora 业务逻辑。Wavora 只保存播放器特有的视觉编排和产品状态。
