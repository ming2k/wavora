#!/usr/bin/env bash
# Bump the project version in one shot.
#
# Usage:
#   packaging/bump-version.sh 0.0.3
#   packaging/bump-version.sh 0.0.3 2026-07-21   # custom date
#
# Updates:
#   1. Cargo.toml          — [workspace.package].version (source of truth)
#   2. Cargo.lock          — via `cargo update -w`
#   3. metainfo.xml        — prepends a <release> entry
#
# Does NOT touch CHANGELOG.md (that's editorial content only you can write).
# After running this, add the CHANGELOG entry, commit, and tag.
set -euo pipefail

if [ $# -lt 1 ]; then
    echo "usage: $0 <version> [date]" >&2
    exit 1
fi

VERSION="$1"
DATE="${2:-$(date +%F)}"
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
METAINFO="$ROOT/packaging/io.github.ming2k.Wavora.metainfo.xml"

# --- 1. Cargo.toml: only the [workspace.package] version line ---
python3 - "$ROOT/Cargo.toml" "$VERSION" <<'PY'
import re, sys
path, version = sys.argv[1], sys.argv[2]
src = open(path).read()
src = re.sub(
    r'(\[workspace\.package\][^\[]*?version\s*=\s*)"[^"]*"',
    rf'\1"{version}"',
    src,
    count=1,
    flags=re.DOTALL,
)
open(path, 'w').write(src)
PY

# --- 2. Cargo.lock: let cargo resolve the workspace ---
(cd "$ROOT" && cargo update -w)

# --- 3. metainfo.xml: prepend <release> if this version isn't listed ---
if ! grep -q "version=\"$VERSION\"" "$METAINFO"; then
    python3 - "$METAINFO" "$VERSION" "$DATE" <<'PY'
import sys
path, version, date = sys.argv[1], sys.argv[2], sys.argv[3]
lines = open(path).read().splitlines()
out = []
for line in lines:
    out.append(line)
    if '<releases>' in line:
        out.append(f'    <release version="{version}" date="{date}"/>')
open(path, 'w').write('\n'.join(out) + '\n')
PY
fi

echo "Bumped to $VERSION ($DATE)."
echo "Next: edit CHANGELOG.md, then commit + tag:"
echo "  git commit -am 'chore: bump version to $VERSION'"
echo "  git tag -a v$VERSION -m 'v$VERSION'"
