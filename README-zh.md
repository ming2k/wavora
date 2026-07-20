# Wavora

[English](README.md) | **简体中文**

[![CI](https://github.com/ming2k/wavora/actions/workflows/ci.yml/badge.svg)](https://github.com/ming2k/wavora/actions/workflows/ci.yml)
[![Release](https://github.com/ming2k/wavora/actions/workflows/release.yml/badge.svg)](https://github.com/ming2k/wavora/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

本地优先、视觉沉浸的音乐播放器。原生 Wayland + Vulkan 界面由
[Optics](https://github.com/ming2k/optics) 图形栈渲染，本地解码音频，
配合音频驱动的视觉舞台。

## 演示

https://github.com/ming2k/wavora/raw/main/docs/media/demo.mp4

## 下载

预编译二进制发布在 [Releases](https://github.com/ming2k/wavora/releases) 页面。
每个压缩包静态链接 Optics 并附带 `install.sh`。

## 构建

需要 Rust ≥ 1.92、meson、Vulkan 1.3、Wayland 和 GStreamer 1.20+。

```bash
# 1. 构建并安装 Optics（一次性）
git clone https://github.com/ming2k/optics.git && cd optics
meson setup build --prefix=/usr/local -Ddefault_library=static -Dexamples=false
meson compile -C build && sudo meson install -C build

# 2. 构建 Wavora
cd .. && git clone https://github.com/ming2k/wavora.git && cd wavora
PKG_CONFIG_ALL_STATIC=1 cargo build --release
```

## 运行

```bash
cargo run --release -- ~/Music
cargo run --release -- --visuals --preset=0
```

启动参数：`--visuals` `--library` `--playlists` `--lyrics` `--preset=0..10`

## 许可证

[MIT](LICENSE)
