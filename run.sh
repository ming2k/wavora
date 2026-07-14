#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
OPTICS="$(cd -- "$ROOT/../optics" && pwd)"

export IRIS_SOURCE_DIR="$OPTICS"
export IRIS_BUILD_DIR="$OPTICS/build"
export LENS_SOURCE_DIR="$OPTICS"
export LENS_BUILD_DIR="$OPTICS/build"
export FLUX_SOURCE_DIR="$OPTICS"
export FLUX_BUILD_DIR="$OPTICS/build"
export LD_LIBRARY_PATH="$OPTICS/build/libs/iris:$OPTICS/build/libs/lens:$OPTICS/build/libs/flux:${LD_LIBRARY_PATH:-}"

exec cargo run --release -- "$@"

