#!/usr/bin/env bash
# The same steps as ./check.sh, for cloud workspaces where the pinned Rust or Playwright
# browser setup differs from a laptop. The owner never runs this; use ./check.sh there.
#
# Workarounds, all local to the workspace:
#   - Rust: the installed stable toolchain by default (set RUSTUP_TOOLCHAIN to override).
#   - wasm32: if the prebuilt std is missing, the standard library is built from source
#     (-Zbuild-std). The sources for the exact compiler commit are fetched once from GitHub.
#   - Browser: if /opt/pw-browsers has a Chromium (Claude's cloud), link it under the build
#     number this project's Playwright expects. Otherwise install Chromium with Playwright
#     (Cursor cloud and similar).
#
# Usage: tools/cloud-check.sh [repo-root]
set -euo pipefail
ROOT="$(cd "${1:-$(dirname "${BASH_SOURCE[0]}")/..}" && pwd)"
export RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-stable}"
CACHE="${SEATLOCK_CLOUD_CACHE:-$HOME/.cache/seatlock-cloud}"
mkdir -p "$CACHE"

# 1. Standard library sources for wasm32, matched to this compiler.
SYSROOT="$(rustc --print sysroot)"
SRC="$SYSROOT/lib/rustlib/src/rust/library"
if [[ ! -f "$SRC/std/Cargo.toml" || ! -f "$SRC/backtrace/Cargo.toml" ]]; then
  SHA="$(rustc -vV | awk '/^commit-hash:/{print $2}')"
  LIB="$CACHE/rust-$SHA"
  if [[ ! -f "$LIB/library/backtrace/Cargo.toml" ]]; then
    echo "==> fetching standard library sources for rustc $SHA"
    rm -rf "$LIB" && mkdir -p "$LIB" && cd "$LIB"
    git init -q && git remote add origin https://github.com/rust-lang/rust
    git sparse-checkout init --cone && git sparse-checkout set library
    GIT_LFS_SKIP_SMUDGE=1 git fetch -q --depth 1 --filter=blob:none origin "$SHA"
    git checkout -q FETCH_HEAD
    BT="$(git ls-files -s library/backtrace | awk '{print $2}')"
    (cd library/backtrace && git init -q && git remote add origin https://github.com/rust-lang/backtrace-rs \
      && git fetch -q --depth 1 origin "$BT" && git checkout -q FETCH_HEAD)
  fi
  mkdir -p "$SYSROOT/lib/rustlib/src/rust"
  ln -sfn "$LIB/library" "$SRC"
fi

# 2. Chromium: reuse a preinstalled tree when present, otherwise let Playwright install.
link_chromium() {
  local want have pw="$CACHE/pw"
  if ls -d /opt/pw-browsers/chromium-* >/dev/null 2>&1; then
    want="$(python3 -c "import json,sys;d=json.load(open(sys.argv[1]));print([b['revision'] for b in d['browsers'] if b['name']=='chromium'][0])" \
      "$ROOT/web/node_modules/playwright-core/browsers.json")"
    have="$(ls -d /opt/pw-browsers/chromium-* | sed 's/.*chromium-//' | sort -n | tail -1)"
    mkdir -p "$pw"
    ln -sfn "/opt/pw-browsers/chromium-$have" "$pw/chromium-$want"
    ln -sfn "/opt/pw-browsers/chromium_headless_shell-$have" "$pw/chromium_headless_shell-$want"
    for f in /opt/pw-browsers/ffmpeg-*; do ln -sfn "$f" "$pw/$(basename "$f")"; done
    export PLAYWRIGHT_BROWSERS_PATH="$pw"
  else
    unset PLAYWRIGHT_BROWSERS_PATH || true
    npx playwright install chromium
  fi
}

cd "$ROOT/engine"
echo "==> rustfmt";  cargo fmt --check
echo "==> clippy";   cargo clippy -q --workspace --all-targets -- -D warnings
echo "==> cargo test"
cargo test -q --workspace 2>&1 | awk '/test result/{p+=$4; f+=$6} /FAILED|panicked/{print} END {print "engine tests passed", p, "failed", f; if (f > 0 || p == 0) exit 1}'
echo "==> TypeScript bindings"
cargo test -q -p seatlock-core --features typescript export_typescript_bindings -- --exact >/dev/null 2>&1
echo "==> wasm build"
RUSTC_BOOTSTRAP=1 cargo build -q --target wasm32-unknown-unknown --release -Zbuild-std=std,panic_abort -p seatlock-wasm -p seatlock-rt

cd "$ROOT"
mkdir -p web/public/examples
cp engine/target/wasm32-unknown-unknown/release/seatlock_wasm.wasm web/public/
cp engine/target/wasm32-unknown-unknown/release/seatlock_rt.wasm web/public/
cp designs/examples/*.seatlock.json web/public/examples/

cd web
[[ -d node_modules ]] || npm ci --no-audit --no-fund
link_chromium
echo "==> web checks"
npm run -s typecheck
npm run -s test 2>&1 | tail -3
npm run -s build 2>&1 | tail -1
# Keep the full log so a red suite cannot look green when only the tail is shown.
CI=1 npm run -s test:e2e 2>&1 | tee /tmp/seatlock-e2e.log | tail -8
if grep -E '^[[:space:]]*[0-9]+ failed' /tmp/seatlock-e2e.log >/dev/null; then
  echo "Browser e2e failed (see /tmp/seatlock-e2e.log)." >&2
  exit 1
fi
echo "All checks passed (cloud workspace: stable $(rustc --version | cut -d' ' -f2), std built from source)."
