#!/usr/bin/env bash
# install.sh — install dd_pantheon from a GitHub Release, or build from source.
#
# Curl (no Rust required) — picks the tarball for this OS/arch:
#
#   curl -fsSL https://raw.githubusercontent.com/ldnddev/dd_pantheon/main/install.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/ldnddev/dd_pantheon/main/install.sh | bash -s -- uninstall
#
# From a clone:
#
#   ./install.sh                # cargo release build if cargo is on PATH, else the release tarball
#   ./install.sh install        # same
#   ./install.sh --from-release # always download the GitHub package
#   ./install.sh --from-source  # always cargo build
#   ./install.sh uninstall      # remove the binary + default theme (not app config)
#   ./install.sh --help
#
# Override defaults via env:
#   PREFIX=$HOME/.local            # binary at $PREFIX/bin/dd_pantheon
#   BINDIR=/path/to/bin            # wins over PREFIX
#   XDG_CONFIG_HOME=$HOME/.config  # theme at …/ldnddev/dd_pantheon_theme.yml
#   VERSION=latest                 # or v0.2.2 / 0.2.2
#   DD_PANTHEON_REPO=ldnddev/dd_pantheon
#   GITHUB_TOKEN=…                 # optional; raises GitHub API rate limits
#
# Re-run safe: the binary is overwritten; an existing theme file is left alone.
# App state (~/.config/ldnddev/dd_pantheon/) is never written or deleted here.
# Safe to pipe from curl: the script never reads stdin.

set -euo pipefail

APP_NAME="dd_pantheon"
THEME_FILE_NAME="dd_pantheon_theme.yml"
THEME_DIR_NAME="ldnddev"
MIN_RUST_VERSION="1.85.0"
DEFAULT_REPO="ldnddev/dd_pantheon"
REPO="${DD_PANTHEON_REPO:-$DEFAULT_REPO}"
VERSION="${VERSION:-latest}"

usage() {
  cat <<'USAGE'
Install dd_pantheon for this machine.

Curl (prebuilt package, no Rust):
  curl -fsSL https://raw.githubusercontent.com/ldnddev/dd_pantheon/main/install.sh | bash
  curl -fsSL https://raw.githubusercontent.com/ldnddev/dd_pantheon/main/install.sh | bash -s -- uninstall

From a clone:
  ./install.sh                  cargo build if available, else GitHub Release
  ./install.sh --from-release   GitHub Release tarball for this OS/arch
  ./install.sh --from-source    cargo build --release
  ./install.sh uninstall        remove binary + default theme (not app config)

Flags:
  install          default
  uninstall        remove the install
  --from-release   download the matching GitHub package
  --from-source    build with cargo (needs a clone or git)
  --debug          cargo build without --release
  --no-build       reuse target/{release,debug}/dd_pantheon from a clone
  --print-target   print the package target triple and exit
  -h, --help       this help

Env:
  PREFIX, BINDIR, XDG_CONFIG_HOME, VERSION, DD_PANTHEON_REPO, GITHUB_TOKEN
USAGE
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

# True when this script lives next to Cargo.toml (a real clone, not `curl | bash`).
in_clone() {
  local src="${BASH_SOURCE[0]:-}"
  [ -n "$src" ] && [ -f "$src" ] || return 1
  local dir
  dir="$(cd -- "$(dirname -- "$src")" && pwd)"
  [ -f "$dir/Cargo.toml" ] && [ -f "$dir/src/main.rs" ]
}

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$arch" in
    x86_64 | amd64) arch="x86_64" ;;
    aarch64 | arm64) arch="aarch64" ;;
    *) fail "unsupported CPU: $(uname -m) (need x86_64 or aarch64/arm64)" ;;
  esac
  case "$os" in
    Linux) printf '%s-unknown-linux-gnu\n' "$arch" ;;
    Darwin) printf '%s-apple-darwin\n' "$arch" ;;
    *) fail "unsupported OS: $os (Linux and macOS only; Windows is out of scope)" ;;
  esac
}

is_release_tag() {
  case "$1" in
    v[0-9]*.[0-9]*.[0-9]*) return 0 ;;
    v[0-9]*.[0-9]*) return 0 ;;
    *) return 1 ;;
  esac
}

normalize_tag() {
  local v="$1"
  case "$v" in
    latest) printf 'latest\n' ;;
    v*) printf '%s\n' "$v" ;;
    *) printf 'v%s\n' "$v" ;;
  esac
}

http_get() {
  local url="$1"
  local out="${2:-}"
  local token="${GITHUB_TOKEN:-${GH_TOKEN:-}}"
  if command -v curl >/dev/null 2>&1; then
    local -a cmd=(curl -fsSL -H "User-Agent: ${APP_NAME}-install")
    if [ -n "$token" ]; then
      cmd+=(-H "Authorization: Bearer ${token}")
    fi
    if [ -n "$out" ]; then
      cmd+=(-o "$out")
    fi
    cmd+=("$url")
    "${cmd[@]}"
  elif command -v wget >/dev/null 2>&1; then
    local -a cmd=(wget -q --user-agent="${APP_NAME}-install")
    if [ -n "$token" ]; then
      cmd+=(--header="Authorization: Bearer ${token}")
    fi
    if [ -n "$out" ]; then
      cmd+=(-O "$out")
    else
      cmd+=(-O -)
    fi
    cmd+=("$url")
    "${cmd[@]}"
  else
    fail "need curl or wget to download packages"
  fi
}

resolve_latest_tag() {
  local json tag loc
  json="$(http_get "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null || true)"
  tag="$(printf '%s\n' "$json" | sed -n 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1)"
  if is_release_tag "$tag"; then
    printf '%s\n' "$tag"
    return 0
  fi
  if command -v curl >/dev/null 2>&1; then
    loc="$(curl -fsSI "https://github.com/${REPO}/releases/latest" 2>/dev/null | tr -d '\r' | awk 'tolower($1)=="location:"{print $2; exit}' || true)"
    tag="${loc##*/}"
    if is_release_tag "$tag"; then
      printf '%s\n' "$tag"
      return 0
    fi
  fi
  fail "no GitHub Release found at https://github.com/${REPO}/releases — push a v*.*.* tag (the release workflow attaches packages) or run ./install.sh --from-source"
}

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    fail "need sha256sum or shasum to verify the package"
  fi
}

find_in_tree() {
  local root="$1"
  local name="$2"
  local f
  while IFS= read -r f; do
    if [ -n "$f" ]; then
      printf '%s\n' "$f"
      return 0
    fi
  done <<EOF
$(find "$root" -type f -name "$name")
EOF
  return 1
}

DO_BUILD=1
BUILD_PROFILE="release"
UNINSTALL=0
FROM_RELEASE=0
FROM_SOURCE=0
PRINT_TARGET=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    install) ;;
    uninstall | -uninstall | --uninstall) UNINSTALL=1 ;;
    --from-release | --prebuilt) FROM_RELEASE=1 ;;
    --from-source | --build) FROM_SOURCE=1 ;;
    --debug) BUILD_PROFILE="debug" ;;
    --no-build) DO_BUILD=0 ;;
    --print-target) PRINT_TARGET=1 ;;
    -h | --help)
      usage
      exit 0
      ;;
    *) fail "unknown option: $1" ;;
  esac
  shift
done

if [ "$FROM_RELEASE" -eq 1 ] && [ "$FROM_SOURCE" -eq 1 ]; then
  fail "use only one of --from-release and --from-source"
fi

TARGET="$(detect_target)"
if [ "$PRINT_TARGET" -eq 1 ]; then
  printf '%s\n' "$TARGET"
  exit 0
fi

SCRIPT_DIR=""
if in_clone; then
  SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
fi

if [ -n "${BINDIR:-}" ]; then
  INSTALL_DIR="$BINDIR"
elif [ -n "${PREFIX:-}" ]; then
  INSTALL_DIR="${PREFIX%/}/bin"
else
  INSTALL_DIR="$HOME/.local/bin"
fi

CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
THEME_DIR="${CONFIG_HOME%/}/${THEME_DIR_NAME}"
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

install_theme() {
  local source="$1"
  [ -f "$source" ] || return 0
  mkdir -p "$THEME_DIR"
  if [ -f "$THEME_TARGET" ]; then
    info "theme already exists at ${THEME_TARGET} — leaving it alone"
  else
    install -m 0644 "$source" "$THEME_TARGET"
    info "installed default theme → ${THEME_TARGET}"
  fi
}

finish() {
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
}

install_binary() {
  local source="$1"
  [ -f "$source" ] || fail "package did not contain ${APP_NAME}"
  chmod +x "$source"
  mkdir -p "$INSTALL_DIR"
  install -m 0755 "$source" "$INSTALL_DIR/$APP_NAME"
  info "installed ${INSTALL_DIR}/${APP_NAME}"
}

install_from_release() {
  command -v tar >/dev/null 2>&1 || fail "tar is required to unpack the package"
  command -v install >/dev/null 2>&1 || fail "install(1) was not found."

  local tag
  tag="$(normalize_tag "$VERSION")"
  if [ "$tag" = "latest" ]; then
    tag="$(resolve_latest_tag)"
  fi
  if [ "$(uname -s)" = Linux ] && { [ -f /etc/alpine-release ] || ldd /bin/sh 2>&1 | grep -qi musl; }; then
    info "warning: this looks like musl libc; published packages are glibc. Use --from-source on Alpine."
  fi
  info "installing ${APP_NAME} ${tag} for ${TARGET}"

  local asset="dd_pantheon-${tag}-${TARGET}.tar.gz"
  local url="https://github.com/${REPO}/releases/download/${tag}/${asset}"
  local work
  work="$(mktemp -d)"
  trap 'rm -rf "$work"' EXIT

  info "downloading ${url}"
  if ! http_get "$url" "$work/$asset"; then
    fail "no package ${asset} in ${tag}. Supported targets: x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu, x86_64-apple-darwin, aarch64-apple-darwin"
  fi

  local sumfile="$work/${asset}.sha256"
  if http_get "${url}.sha256" "$sumfile" 2>/dev/null; then
    local expect got
    expect="$(awk '{print $1}' "$sumfile" | head -n1)"
    got="$(sha256_of "$work/$asset")"
    [ -n "$expect" ] || fail "checksum file for ${asset} was empty"
    if [ "$expect" != "$got" ]; then
      fail "SHA-256 mismatch for ${asset} (expected ${expect}, got ${got})"
    fi
    info "checksum ok"
  else
    info "no ${asset}.sha256 attached to the release — skipping verify"
  fi

  gzip -t "$work/$asset" 2>/dev/null || fail "download was not a gzip archive (wrong asset or HTML error page)"
  tar -xzf "$work/$asset" -C "$work"

  local bin theme
  bin="$(find_in_tree "$work" "$APP_NAME")"
  theme="$(find_in_tree "$work" "$THEME_FILE_NAME")"
  [ -n "$bin" ] || fail "tarball did not contain ${APP_NAME}"

  install_binary "$bin"
  if [ -n "$theme" ]; then
    install_theme "$theme"
  fi
  finish
}

install_from_source() {
  local root="${1:-}"
  if [ -z "$root" ]; then
    fail "No Cargo.toml here — clone the repo or omit --from-source to download a package."
  fi
  cd "$root"
  [ -f Cargo.toml ] || fail "No Cargo.toml in $root"
  local theme_source="$root/$THEME_FILE_NAME"
  [ -f "$theme_source" ] || fail "theme file not found at $theme_source"

  command -v cargo >/dev/null 2>&1 || fail "cargo was not found. Install Rust from https://rustup.rs/ or rerun without --from-source."
  command -v rustc >/dev/null 2>&1 || fail "rustc was not found."
  command -v install >/dev/null 2>&1 || fail "install(1) was not found."

  local rust_version
  rust_version="$(rustc --version | awk '{print $2}')"
  if ! version_ge "$rust_version" "$MIN_RUST_VERSION"; then
    fail "Rust ${MIN_RUST_VERSION}+ is required (edition 2024); found ${rust_version}."
  fi

  if [ "$DO_BUILD" -eq 1 ]; then
    info "building ${APP_NAME} (${BUILD_PROFILE})"
    if [ "$BUILD_PROFILE" = "release" ]; then
      cargo build --release
    else
      cargo build
    fi
  fi

  local source_bin
  if [ "$BUILD_PROFILE" = "release" ]; then
    source_bin="$root/target/release/$APP_NAME"
  else
    source_bin="$root/target/debug/$APP_NAME"
  fi
  [ -x "$source_bin" ] || fail "built binary not found at $source_bin"

  info "installing to ${INSTALL_DIR}/${APP_NAME}"
  install_binary "$source_bin"
  install_theme "$theme_source"
  finish
}

if [ "$FROM_SOURCE" -eq 1 ]; then
  install_from_source "${SCRIPT_DIR}"
elif [ "$FROM_RELEASE" -eq 1 ]; then
  install_from_release
elif [ -n "$SCRIPT_DIR" ] && command -v cargo >/dev/null 2>&1 && [ "$DO_BUILD" -eq 1 ]; then
  install_from_source "$SCRIPT_DIR"
elif [ -n "$SCRIPT_DIR" ] && [ "$DO_BUILD" -eq 0 ]; then
  install_from_source "$SCRIPT_DIR"
else
  install_from_release
fi
