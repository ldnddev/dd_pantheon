#!/usr/bin/env bash
# install.sh — build + install dd_pantheon, or remove an existing install.
#
# Usage:
#   ./install.sh                # build (release) and install
#   ./install.sh install        # same as above
#   ./install.sh uninstall      # remove the binary + default theme (not app config)
#   ./install.sh --help
#
# Override defaults via env:
#   PREFIX=$HOME/.local            # binary at $PREFIX/bin/dd_pantheon
#   BINDIR=/path/to/bin            # wins over PREFIX
#   XDG_CONFIG_HOME=$HOME/.config  # theme at …/ldnddev/dd_pantheon_theme.yml
#
# Re-run safe: the binary is overwritten; an existing theme file is left alone.
# App state (~/.config/ldnddev/dd_pantheon/) is never written or deleted here.

set -euo pipefail

APP_NAME="dd_pantheon"
THEME_FILE_NAME="dd_pantheon_theme.yml"
THEME_DIR_NAME="ldnddev"
MIN_RUST_VERSION="1.85.0"

usage() {
  sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'
}

fail() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

info() {
  printf '==> %s\n' "$*"
}

version_ge() {
  local version="$1"
  local required="$2"
  [ "$(printf '%s\n%s\n' "$required" "$version" | sort -V | head -n1)" = "$required" ]
}

DO_BUILD=1
BUILD_PROFILE="release"
UNINSTALL=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    install) ;;
    uninstall | -uninstall | --uninstall) UNINSTALL=1 ;;
    --debug) BUILD_PROFILE="debug" ;;
    --no-build) DO_BUILD=0 ;;
    -h | --help)
      usage
      exit 0
      ;;
    *) fail "unknown option: $1" ;;
  esac
  shift
done

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

if [ -n "${BINDIR:-}" ]; then
  INSTALL_DIR="$BINDIR"
elif [ -n "${PREFIX:-}" ]; then
  INSTALL_DIR="${PREFIX%/}/bin"
else
  INSTALL_DIR="$HOME/.local/bin"
fi

CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
THEME_DIR="${CONFIG_HOME%/}/${THEME_DIR_NAME}"
THEME_SOURCE="$SCRIPT_DIR/$THEME_FILE_NAME"
THEME_TARGET="$THEME_DIR/$THEME_FILE_NAME"
APP_CONFIG_DIR="${CONFIG_HOME%/}/${THEME_DIR_NAME}/${APP_NAME}"

if [ "$UNINSTALL" -eq 1 ]; then
  info "uninstalling ${APP_NAME}"
  if [ -e "$INSTALL_DIR/$APP_NAME" ] || [ -L "$INSTALL_DIR/$APP_NAME" ]; then
    rm -f "$INSTALL_DIR/$APP_NAME"
    info "removed ${INSTALL_DIR}/${APP_NAME}"
  else
    info "binary not found at ${INSTALL_DIR}/${APP_NAME}"
  fi
  if [ -e "$THEME_TARGET" ] || [ -L "$THEME_TARGET" ]; then
    rm -f "$THEME_TARGET"
    info "removed ${THEME_TARGET}"
  else
    info "theme not found at ${THEME_TARGET}"
  fi
  info "left ${APP_CONFIG_DIR} in place (layout, history, sites overlay)"
  info "done"
  exit 0
fi

[ -f Cargo.toml ] || fail "No Cargo.toml in $SCRIPT_DIR — run install.sh from the repo root."
[ -f "$THEME_SOURCE" ] || fail "theme file not found at $THEME_SOURCE"

command -v cargo >/dev/null 2>&1 || fail "cargo was not found. Install Rust from https://rustup.rs/"
command -v rustc >/dev/null 2>&1 || fail "rustc was not found."
command -v install >/dev/null 2>&1 || fail "install(1) was not found."

RUST_VERSION="$(rustc --version | awk '{print $2}')"
if ! version_ge "$RUST_VERSION" "$MIN_RUST_VERSION"; then
  fail "Rust ${MIN_RUST_VERSION}+ is required (edition 2024); found ${RUST_VERSION}."
fi

if [ "$DO_BUILD" -eq 1 ]; then
  info "building ${APP_NAME} (${BUILD_PROFILE})"
  if [ "$BUILD_PROFILE" = "release" ]; then
    cargo build --release
  else
    cargo build
  fi
fi

if [ "$BUILD_PROFILE" = "release" ]; then
  SOURCE_BIN="$SCRIPT_DIR/target/release/$APP_NAME"
else
  SOURCE_BIN="$SCRIPT_DIR/target/debug/$APP_NAME"
fi
[ -x "$SOURCE_BIN" ] || fail "built binary not found at $SOURCE_BIN"

info "installing to ${INSTALL_DIR}/${APP_NAME}"
mkdir -p "$INSTALL_DIR"
install -m 0755 "$SOURCE_BIN" "$INSTALL_DIR/$APP_NAME"

mkdir -p "$THEME_DIR"
if [ -f "$THEME_TARGET" ]; then
  info "theme already exists at ${THEME_TARGET} — leaving it alone"
else
  install -m 0644 "$THEME_SOURCE" "$THEME_TARGET"
  info "installed default theme → ${THEME_TARGET}"
fi

case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    cat <<PATH_NOTE

${INSTALL_DIR} is not on your PATH.
Add this line to your shell profile:

  export PATH="${INSTALL_DIR}:\$PATH"

PATH_NOTE
    ;;
esac

info "done. Try:  ${APP_NAME} --demo"
info "app config (created on first run): ${APP_CONFIG_DIR}/"
