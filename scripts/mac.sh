#!/bin/bash
# Meetily ID — run (dev) or build (release .app) on Apple Silicon.
#
#   scripts/mac.sh dev     start the dev app detached; log in $LOG_DIR/dev.log
#   scripts/mac.sh build   build a release .app/.dmg (target/release/bundle/)
#
# Handles two macOS-specific build issues:
# - llama-helper sidecar must be built and staged before Tauri (docs/BUILDING.md).
# - Homebrew links OpenSSL 4 by default; llama.cpp's cpp-httplib does not compile against it,
#   so OpenSSL 3 is used when it is installed.
set -euo pipefail

MODE="${1:-}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LOG_DIR="${LOG_DIR:-$ROOT/.logs}"
HOST="$(rustc -vV | awk '/^host:/{print $2}')"
SIDECAR="$ROOT/frontend/src-tauri/binaries/llama-helper-$HOST"

usage() {
  echo "usage: scripts/mac.sh dev|build" >&2
  exit 2
}

use_openssl3() {
  local prefix
  prefix="$(brew --prefix openssl@3 2>/dev/null || true)"
  if [ -n "$prefix" ] && [ -d "$prefix" ]; then
    export OPENSSL_ROOT_DIR="$prefix"
    echo "OpenSSL: $prefix"
  else
    echo "OpenSSL 3 not found (brew install openssl@3); building against the default OpenSSL" >&2
  fi
}

stage_sidecar() { # profile: debug|release
  local profile=$1 flag=""
  [ "$profile" = "release" ] && flag="--release"
  if [ "$profile" = "debug" ] && [ -x "$SIDECAR" ]; then
    echo "llama-helper: already staged"
    return
  fi
  echo "llama-helper: building ($profile, metal)"
  (cd "$ROOT" && cargo build -p llama-helper --features metal $flag)
  mkdir -p "$(dirname "$SIDECAR")"
  cp -f "$ROOT/target/$profile/llama-helper" "$SIDECAR"
}

[ -n "$MODE" ] || usage
use_openssl3

case "$MODE" in
  dev)
    stage_sidecar debug
    mkdir -p "$LOG_DIR"
    cd "$ROOT/frontend"
    [ -d node_modules ] || pnpm install
    # Detached so the app outlives the terminal or agent session that started it.
    nohup pnpm tauri:dev > "$LOG_DIR/dev.log" 2>&1 < /dev/null &
    disown
    echo "dev app starting (pid $!), log: $LOG_DIR/dev.log"
    ;;
  build)
    stage_sidecar release
    cd "$ROOT/frontend"
    [ -d node_modules ] || pnpm install
    # Only the .app: DMG creation scripts Finder and fails in headless sessions.
    feature="$(node scripts/auto-detect-gpu.js 2>/dev/null | tail -1 || true)"
    if [ -n "$feature" ] && [ "$feature" != "none" ]; then
      pnpm tauri build --bundles app -- --features "$feature"
    else
      pnpm tauri build --bundles app
    fi
    echo "app: $ROOT/target/release/bundle/macos/"
    ;;
  *)
    usage
    ;;
esac
