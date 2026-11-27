#!/usr/bin/env bash
# install.sh — install dd_pantheon from a GitHub Release, or build from source.
#
# Curl (no Rust required for the terminal package):
#
#   curl -fsSL https://raw.githubusercontent.com/ldnddev/dd_pantheon/main/install.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/ldnddev/dd_pantheon/main/install.sh | bash -s -- --both
#   curl -fsSL https://raw.githubusercontent.com/ldnddev/dd_pantheon/main/install.sh | bash -s -- --uninstall
#
# From a clone:
#
#   ./install.sh                 # ask: terminal, window, or both
#   ./install.sh --tui           # dd_pantheon
#   ./install.sh --gui           # dd_pantheon-gui
#   ./install.sh --both
#   ./install.sh --from-release  # GitHub package (terminal binary; window needs --from-source)
#   ./install.sh --from-source   # cargo build
#   ./install.sh --uninstall     # ask which program to remove
#   ./install.sh --help
#
# Override defaults via env:
#   PREFIX=$HOME/.local            # binaries at $PREFIX/bin
#   BINDIR=/path/to/bin            # wins over PREFIX
#   XDG_CONFIG_HOME=$HOME/.config  # theme at …/ldnddev/dd_pantheon_theme.yml
#   XDG_DATA_HOME=$HOME/.local/share
#   VERSION=latest                 # or v0.3.10 / 0.3.10
#   DD_PANTHEON_REPO=ldnddev/dd_pantheon
#   GITHUB_TOKEN=…                 # optional; raises GitHub API rate limits
#   INSTALL_GUI=1                  # same as --both, when no --tui/--gui/--both flag
#
# On Linux, install writes a launcher for each binary it installs. The terminal
# launcher is Terminal=true. The window launcher is Terminal=false with
# StartupWMClass=dev.ldnddev.dd_pantheon. $XDG_DATA_HOME/applications wins when
# that variable is set. Otherwise a binary under $HOME uses
# ~/.local/share/applications, and a prefix outside $HOME uses
# $PREFIX/share/applications (or the parent of BINDIR). macOS has no desktop file.
#
# Uninstall removes only the chosen binary and its launcher. It leaves
# dd_pantheon_theme.yml and ~/.config/ldnddev/dd_pantheon alone. Pass the same
# PREFIX, BINDIR, and XDG_DATA_HOME as the install.
#
# Re-run safe: binaries and launchers are overwritten. An existing theme file
# is left alone, and a window-only install does not write one. The theme file
# is copied only when it is missing, and only for the terminal or both.
# App state (~/.config/ldnddev/dd_pantheon/) is never written or deleted here.
# Safe to pipe from curl: the choice prompt reads /dev/tty, not stdin.
# With no terminal to answer, install selects the terminal app and uninstall
# removes both. dd_pantheon-gui needs Vulkan and Wayland or X11.

set -euo pipefail

APP_NAME="dd_pantheon"
GUI_NAME="dd_pantheon-gui"
APP_ID="dev.ldnddev.dd_pantheon"
THEME_FILE_NAME="dd_pantheon_theme.yml"
THEME_DIR_NAME="ldnddev"
MIN_RUST_VERSION="1.85.0"
DEFAULT_REPO="ldnddev/dd_pantheon"
REPO="${DD_PANTHEON_REPO:-$DEFAULT_REPO}"
VERSION="${VERSION:-latest}"

# ldnddev wordmark, half-block art of the white-on-black mark.
# Printed only on a terminal so a captured log stays plain text.
print_mark() {
  [[ -t 1 ]] || return 0
  printf '\033[40;97m'
  cat <<'EOF'

█▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀█
█  ██                ██                         ██  █
█  ██                ██                         ██  █
█  ██         ▄▄▄▄▄▄ ██     ▄▄▄▄▄▄       ▄▄▄▄▄▄ ██  █
█  ██       ▄██▀▀▀▀▀███  ▄██████████▄  ▄██▀▀▀▀▀███  █
█  ██      ██▀       ██  ██        ██ ██▀       ██  █
█  ██      ██        ██  ██        ██ ██        ██  █
█  ██      ▀█▄       ██  ██        ██ ▀█▄       ██  █
█  ▀█▄▄▄▄▄  ▀██▄▄▄▄██▀   ██        ██  ▀██▄▄▄▄██▀   █
█   ▀▀▀▀▀▀    ▀▀▀▀▀▀     ▀▀        ▀▀    ▀▀▀▀▀▀     █
█                                                   █
█                     ██                            █
█                     ██                            █
█              ▄▄▄▄▄▄ ██     ▄▄▄▄▄▄    ▄▄      ▄▄▄  █
█            ▄██▀▀▀▀▀███   ██▀▀▀▀███   ██     ▄██   █
█           ██▀       ██  ██    ██▀ ▄▄ ██    ▄██    █
█           ██        ██  ██  ▄██   ██ ██    ██     █
█           ▀█▄       ██  ██▄██▀   ▄█▀ ██  ▄██      █
█  ▄▄▄▄▄▄▄▄  ▀██▄▄▄▄██▀    ▀███▄▄▄██▀  ██▄██▀       █
█  ▀▀▀▀▀▀▀▀    ▀▀▀▀▀▀        ▀▀▀▀▀▀    ▀▀▀▀         █
▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀
EOF
  printf '\033[0m\n'
}

usage() {
  cat <<'USAGE'
Install dd_pantheon for this machine.

Curl (prebuilt terminal package, no Rust):
  curl -fsSL https://raw.githubusercontent.com/ldnddev/dd_pantheon/main/install.sh | bash
  curl -fsSL https://raw.githubusercontent.com/ldnddev/dd_pantheon/main/install.sh | bash -s -- --tui
  curl -fsSL https://raw.githubusercontent.com/ldnddev/dd_pantheon/main/install.sh | bash -s -- --uninstall

From a clone:
  ./install.sh                  ask: terminal, window, or both
  ./install.sh --tui            install dd_pantheon
  ./install.sh --gui            install dd_pantheon-gui
  ./install.sh --both           install both
  ./install.sh --from-release   GitHub Release tarball for this OS/arch
  ./install.sh --from-source    cargo build --release
  ./install.sh --uninstall      ask which program to remove
  ./install.sh --uninstall --tui
  ./install.sh --uninstall --gui
  ./install.sh --uninstall --both

With no terminal to answer, install selects the terminal app and uninstall
removes both. INSTALL_GUI=1 is the same as --both.

The published release tarball ships the terminal binary. --gui and --both
from a release fail and point at --from-source. A window-only install does
not write dd_pantheon_theme.yml. The theme file is installed only when it
is missing, and only for --tui or --both.

dd_pantheon-gui needs Vulkan (libvulkan1 and a driver) and Wayland or X11.

Flags:
  install          default
  uninstall        remove the chosen binary and launcher
  --tui            terminal app
  --gui            desktop window
  --both           both programs
  --from-release   download the matching GitHub package
  --from-source    build with cargo (needs a clone or git)
  --debug          cargo build without --release
  --no-build       reuse target/{release,debug} binaries from a clone
  --print-target   print the package target triple and exit
  -h, --help       this help

Uninstall leaves dd_pantheon_theme.yml and ~/.config/ldnddev/dd_pantheon.
Pass the same PREFIX, BINDIR, and XDG_DATA_HOME as the install.

Env:
  PREFIX, BINDIR, XDG_CONFIG_HOME, XDG_DATA_HOME, VERSION, DD_PANTHEON_REPO,
  GITHUB_TOKEN, INSTALL_GUI=1
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

# Tags must be vMAJOR.MINOR.PATCH with an optional prerelease suffix.
# The old glob (v[0-9]*.[0-9]*.[0-9]*) also matched v1.2.3/../evil and shell metacharacters.
is_release_tag() {
  printf '%s\n' "$1" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+([.-][A-Za-z0-9]+)*$'
}

is_github_repo() {
  printf '%s\n' "$1" | grep -Eq '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$'
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

# Menu launchers. $XDG_DATA_HOME/applications wins when set. A binary under
# $HOME otherwise goes to ~/.local/share/applications. A prefix outside $HOME
# uses $PREFIX/share/applications, or the parent of BINDIR when PREFIX is unset.
apps_dir() {
  if [ -n "${XDG_DATA_HOME:-}" ]; then
    printf '%s/applications\n' "${XDG_DATA_HOME%/}"
    return 0
  fi
  case "$INSTALL_DIR" in
    "$HOME" | "$HOME"/*)
      printf '%s/.local/share/applications\n' "$HOME"
      ;;
    *)
      if [ -n "${PREFIX:-}" ]; then
        printf '%s/share/applications\n' "${PREFIX%/}"
      else
        printf '%s/share/applications\n' "$(dirname -- "$INSTALL_DIR")"
      fi
      ;;
  esac
}

# Quote one Exec argument. A literal % is %%.
desktop_quote() {
  local s="$1"
  case "$s" in
    *$'\n'*) fail "desktop path contains a newline" ;;
  esac
  s="${s//\\/\\\\}"
  s="${s//\"/\\\"}"
  s="${s//\`/\\\`}"
  s="${s//\$/\\\$}"
  s="${s//%/%%}"
  printf '"%s"' "$s"
}

refresh_desktop_database() {
  local dir="$1"
  if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$dir" >/dev/null 2>&1 || true
  fi
}

write_desktop() {
  local dest="$1"
  local name="$2"
  local generic="$3"
  local comment="$4"
  local bin="$5"
  local terminal="$6"
  local startup="$7"
  local icon="$8"
  local categories="$9"
  local keywords="${10}"
  local wmclass="${11:-}"
  local exec_q
  exec_q="$(desktop_quote "$bin")"
  {
    printf '%s\n' "[Desktop Entry]"
    printf '%s\n' "Type=Application"
    printf '%s\n' "Version=1.0"
    printf '%s\n' "Name=${name}"
    printf '%s\n' "GenericName=${generic}"
    printf '%s\n' "Comment=${comment}"
    printf '%s\n' "Exec=${exec_q}"
    printf '%s\n' "TryExec=${bin}"
    if [ -n "$icon" ]; then
      printf '%s\n' "Icon=${icon}"
    fi
    printf '%s\n' "Terminal=${terminal}"
    printf '%s\n' "Categories=${categories}"
    printf '%s\n' "Keywords=${keywords}"
    printf '%s\n' "StartupNotify=${startup}"
    if [ -n "$wmclass" ]; then
      printf '%s\n' "StartupWMClass=${wmclass}"
    fi
  } >"$dest"
  chmod 644 "$dest"
}

# Refresh a launcher for every binary that is still installed.
install_launchers() {
  if [ "$(uname -s)" != Linux ]; then
    info "desktop launcher skipped on $(uname -s)"
    return 0
  fi
  local dir wrote=0
  dir="$(apps_dir)"
  mkdir -p "$dir"
  if [ -x "${INSTALL_DIR}/${APP_NAME}" ]; then
    write_desktop \
      "${dir}/${APP_NAME}.desktop" \
      "${APP_NAME}" \
      "Pantheon operations" \
      "Operate Pantheon WordPress and Drupal sites in the terminal" \
      "${INSTALL_DIR}/${APP_NAME}" \
      "true" \
      "false" \
      "utilities-terminal" \
      "Development;ConsoleOnly;" \
      "pantheon;wordpress;drupal;terminus;"
    info "installed launcher ${dir}/${APP_NAME}.desktop"
    wrote=1
  fi
  if [ -x "${INSTALL_DIR}/${GUI_NAME}" ]; then
    write_desktop \
      "${dir}/${GUI_NAME}.desktop" \
      "dd_pantheon GUI" \
      "Pantheon operations" \
      "Operate Pantheon WordPress and Drupal sites in a window" \
      "${INSTALL_DIR}/${GUI_NAME}" \
      "false" \
      "true" \
      "" \
      "Development;" \
      "pantheon;wordpress;drupal;terminus;" \
      "${APP_ID}"
    info "installed launcher ${dir}/${GUI_NAME}.desktop"
    wrote=1
  fi
  if [ "$wrote" -eq 1 ]; then
    refresh_desktop_database "$dir"
  fi
}

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

install_named() {
  local source="$1"
  local name="$2"
  [ -f "$source" ] || fail "package did not contain ${name}"
  chmod +x "$source"
  mkdir -p "$INSTALL_DIR"
  install -m 0755 "$source" "${INSTALL_DIR}/${name}"
  info "installed ${INSTALL_DIR}/${name}"
}

wants_tui() {
  [ "$1" != "gui" ]
}

wants_gui() {
  [ "$1" != "tui" ]
}

profile_dir() {
  if [ "$BUILD_PROFILE" = "release" ]; then
    printf 'release\n'
  else
    printf 'debug\n'
  fi
}

built_bin() {
  local root="$1"
  local name="$2"
  local dir
  dir="$(profile_dir)"
  if [ -n "${CARGO_TARGET_DIR:-}" ]; then
    printf '%s/%s/%s\n' "${CARGO_TARGET_DIR%/}" "$dir" "$name"
  else
    printf '%s/target/%s/%s\n' "$root" "$dir" "$name"
  fi
}

cargo_build_pkg() {
  local package="$1"
  if [ "$BUILD_PROFILE" = "release" ]; then
    cargo build --release --locked -p "$package"
  else
    cargo build --locked -p "$package"
  fi
}

finish() {
  local target="$1"
  install_launchers
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
  if wants_gui "$target"; then
    info "${GUI_NAME} needs Vulkan (libvulkan1 and a driver) and Wayland or X11."
  fi
  info "app config (created on first run): ${APP_CONFIG_DIR}/"
  case "$target" in
    tui) info "done. Try:  ${APP_NAME} --demo" ;;
    gui) info "done. Try:  ${GUI_NAME} --demo" ;;
    both) info "done. Try:  ${APP_NAME} --demo    or    ${GUI_NAME} --demo" ;;
  esac
}

# Reject absolute paths, ".." segments, and link members before extract.
# GNU tar 1.35 blocks ".." on extract but still writes symlinks; older tar and bsdtar differ.
assert_archive_members_safe() {
  local archive="$1"
  local listing name type
  listing="$(mktemp)"
  tar -tzf "$archive" >"$listing"
  while IFS= read -r name || [ -n "$name" ]; do
    [ -n "$name" ] || continue
    name="${name#./}"
    case "$name" in
      /*|*\\*)
        rm -f "$listing"
        fail "archive member is absolute: ${name}"
        ;;
    esac
    local rest="${name%/}" seg
    rest="${rest#./}"
    while [ -n "$rest" ]; do
      seg="${rest%%/*}"
      if [ "$seg" = ".." ]; then
        rm -f "$listing"
        fail "archive member escapes the install directory: ${name}"
      fi
      case "$rest" in
        */*) rest="${rest#*/}" ;;
        *) rest="" ;;
      esac
    done
  done <"$listing"
  rm -f "$listing"

  listing="$(mktemp)"
  tar -tvzf "$archive" >"$listing"
  while IFS= read -r name || [ -n "$name" ]; do
    [ -n "$name" ] || continue
    type="${name:0:1}"
    case "$type" in
      - | d) ;;
      *)
        rm -f "$listing"
        fail "archive member is not a regular file or directory: ${name}"
        ;;
    esac
  done <"$listing"
  rm -f "$listing"
}

install_from_release() {
  local target="$1"
  command -v tar >/dev/null 2>&1 || fail "tar is required to unpack the package"
  command -v install >/dev/null 2>&1 || fail "install(1) was not found."
  is_github_repo "$REPO" || fail "DD_PANTHEON_REPO must look like owner/name (got ${REPO})"

  local tag
  tag="$(normalize_tag "$VERSION")"
  if [ "$tag" = "latest" ]; then
    tag="$(resolve_latest_tag)"
  fi
  if [ "$(uname -s)" = Linux ] && { [ -f /etc/alpine-release ] || ldd /bin/sh 2>&1 | grep -qi musl; }; then
    info "warning: this looks like musl libc; published packages are glibc. Use --from-source on Alpine."
  fi
  if ! is_release_tag "$tag"; then
    fail "refusing version '${tag}' (expected vX.Y.Z from a GitHub Release)"
  fi
  info "installing ${APP_NAME} ${tag} for ${TARGET}"

  local asset="dd_pantheon-${tag}-${TARGET}.tar.gz"
  local url="https://github.com/${REPO}/releases/download/${tag}/${asset}"
  # Global so the EXIT trap still sees the path after this function returns
  # (curl | bash + set -u otherwise errors: work: unbound variable).
  INSTALL_WORKDIR="$(mktemp -d)"
  trap 'rm -rf "$INSTALL_WORKDIR"' EXIT

  info "downloading ${url}"
  if ! http_get "$url" "$INSTALL_WORKDIR/$asset"; then
    fail "no package ${asset} in ${tag}. Supported targets: x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu, x86_64-apple-darwin, aarch64-apple-darwin"
  fi

  local sumfile="$INSTALL_WORKDIR/${asset}.sha256"
  if ! http_get "${url}.sha256" "$sumfile"; then
    fail "release ${tag} has no ${asset}.sha256 — refusing to install an unverified package"
  fi
  local expect got
  expect="$(awk '{print $1}' "$sumfile" | head -n1 | tr 'A-F' 'a-f')"
  got="$(sha256_of "$INSTALL_WORKDIR/$asset" | tr 'A-F' 'a-f')"
  printf '%s\n' "$expect" | grep -Eq '^[0-9a-f]{64}$' || fail "checksum file for ${asset} is not a SHA-256 digest"
  if [ "$expect" != "$got" ]; then
    fail "SHA-256 mismatch for ${asset} (expected ${expect}, got ${got})"
  fi
  info "checksum ok"

  gzip -t "$INSTALL_WORKDIR/$asset" 2>/dev/null || fail "download was not a gzip archive (wrong asset or HTML error page)"
  assert_archive_members_safe "$INSTALL_WORKDIR/$asset"
  if tar --help 2>/dev/null | grep -q -- '--no-same-owner'; then
    tar --no-same-owner -xzf "$INSTALL_WORKDIR/$asset" -C "$INSTALL_WORKDIR"
  else
    tar -xzf "$INSTALL_WORKDIR/$asset" -C "$INSTALL_WORKDIR"
  fi

  # find_in_tree returns 1 when the name is absent. Under set -e that aborts
  # a bare $(...) before the message below, so optional names use || true.
  local bin gui theme
  bin="$(find_in_tree "$INSTALL_WORKDIR" "$APP_NAME" || true)"
  gui="$(find_in_tree "$INSTALL_WORKDIR" "$GUI_NAME" || true)"
  theme="$(find_in_tree "$INSTALL_WORKDIR" "$THEME_FILE_NAME" || true)"
  if wants_tui "$target"; then
    [ -n "$bin" ] || fail "tarball did not contain ${APP_NAME}"
  fi
  if wants_gui "$target"; then
    if [ -z "$gui" ]; then
      fail "release ${tag} has no ${GUI_NAME}. Published packages ship the terminal binary. Use --from-source to build the window."
    fi
  fi
  if wants_tui "$target"; then
    install_named "$bin" "$APP_NAME"
    if [ -n "$theme" ]; then
      install_theme "$theme"
    fi
  fi
  if wants_gui "$target"; then
    install_named "$gui" "$GUI_NAME"
  fi
  finish "$target"
}

install_from_source() {
  local root="${1:-}"
  local target="$2"
  if [ -z "$root" ]; then
    fail "No Cargo.toml here — clone the repo or omit --from-source to download a package."
  fi
  cd "$root"
  [ -f Cargo.toml ] || fail "No Cargo.toml in $root"
  [ -f "$root/crates/ldnddev_theme/Cargo.toml" ] || fail "missing in-tree crate $root/crates/ldnddev_theme (this repo does not use an external theme crate)"
  local theme_source="$root/$THEME_FILE_NAME"
  if wants_tui "$target"; then
    [ -f "$theme_source" ] || fail "theme file not found at $theme_source"
  fi
  if wants_gui "$target"; then
    [ -f "$root/crates/dd_pantheon_gui/Cargo.toml" ] || fail "missing GUI crate $root/crates/dd_pantheon_gui"
  fi

  command -v cargo >/dev/null 2>&1 || fail "cargo was not found. Install Rust from https://rustup.rs/ or rerun without --from-source."
  command -v rustc >/dev/null 2>&1 || fail "rustc was not found."
  command -v install >/dev/null 2>&1 || fail "install(1) was not found."

  local rust_version
  rust_version="$(rustc --version | awk '{print $2}')"
  if ! version_ge "$rust_version" "$MIN_RUST_VERSION"; then
    fail "Rust ${MIN_RUST_VERSION}+ is required (edition 2024); found ${rust_version}."
  fi

  if [ "$DO_BUILD" -eq 1 ]; then
    if wants_tui "$target"; then
      info "building ${APP_NAME} (${BUILD_PROFILE})"
      cargo_build_pkg dd_pantheon
    fi
    if wants_gui "$target"; then
      info "building ${GUI_NAME} (${BUILD_PROFILE})"
      cargo_build_pkg dd_pantheon_gui
    fi
  fi

  if wants_tui "$target"; then
    local tui_bin
    tui_bin="$(built_bin "$root" "$APP_NAME")"
    [ -x "$tui_bin" ] || fail "built binary not found at $tui_bin"
    info "installing to ${INSTALL_DIR}/${APP_NAME}"
    install_named "$tui_bin" "$APP_NAME"
    install_theme "$theme_source"
  fi
  if wants_gui "$target"; then
    local gui_bin
    gui_bin="$(built_bin "$root" "$GUI_NAME")"
    [ -x "$gui_bin" ] || fail "built binary not found at $gui_bin"
    info "installing to ${INSTALL_DIR}/${GUI_NAME}"
    install_named "$gui_bin" "$GUI_NAME"
  fi
  finish "$target"
}

uninstall_target() {
  local target="$1"
  local removed=0
  local path
  local -a paths=()
  case "$target" in
    tui)
      paths=(
        "${INSTALL_DIR}/${APP_NAME}"
        "$(apps_dir)/${APP_NAME}.desktop"
      )
      ;;
    gui)
      paths=(
        "${INSTALL_DIR}/${GUI_NAME}"
        "$(apps_dir)/${GUI_NAME}.desktop"
      )
      ;;
    both)
      paths=(
        "${INSTALL_DIR}/${APP_NAME}"
        "${INSTALL_DIR}/${GUI_NAME}"
        "$(apps_dir)/${APP_NAME}.desktop"
        "$(apps_dir)/${GUI_NAME}.desktop"
      )
      ;;
    *) fail "internal: unknown target ${target}" ;;
  esac
  info "uninstalling ${target}"
  for path in "${paths[@]}"; do
    if [ -e "$path" ] || [ -L "$path" ]; then
      rm -f -- "$path"
      info "removed ${path}"
      removed=1
    fi
  done
  if [ "$removed" -eq 0 ]; then
    info "no matching binaries or launchers found"
  fi
  if [ "$(uname -s)" = Linux ]; then
    refresh_desktop_database "$(apps_dir)"
  fi
  info "left ${THEME_TARGET} in place (dd_pantheon_theme.yml)"
  info "left ${APP_CONFIG_DIR} in place (history, sites overlay)"
  info "done"
}

# Read 1, 2, or 3 from the terminal. stdout is tui, gui, or both.
# $1 is the verb shown in the question. $2 is the default number.
# Returns 1 when the terminal cannot be used. main captures stdout, and a
# failed redirect in that capture does not abort under set -e, so every
# /dev/tty failure has to be an explicit status.
ask_choice() {
  local verb=$1 default=$2 answer choice
  while true; do
    {
      printf '%s which program?\n' "$verb"
      printf '%s\n' "  1) terminal (${APP_NAME})"
      printf '%s\n' "  2) desktop window (${GUI_NAME})"
      printf '%s\n' "  3) both"
      printf 'Choice [%s]: ' "$default"
    } >/dev/tty 2>/dev/null || return 1
    if ! IFS= read -r answer </dev/tty 2>/dev/null; then
      printf '\n' >/dev/tty 2>/dev/null || true
      answer=$default
    fi
    choice=$(printf '%s' "$answer" | tr '[:upper:]' '[:lower:]' | tr -d '[:space:]')
    case "$choice" in
      '' ) choice=$default ;;
    esac
    case "$choice" in
      1 | tui | terminal) printf '%s\n' tui; return 0 ;;
      2 | gui | window) printf '%s\n' gui; return 0 ;;
      3 | both) printf '%s\n' both; return 0 ;;
      *) printf '%s\n' "Enter 1, 2, or 3." >/dev/tty 2>/dev/null || return 1 ;;
    esac
  done
}

# The device node is mode rw-rw-rw, so -r/-w stay true with no controlling
# terminal. Opening it is the check that matters.
have_tty() {
  { : <>/dev/tty; } 2>/dev/null
}

choose_target() {
  local action=$1 explicit=$2
  if [[ -n "$explicit" ]]; then
    printf '%s\n' "$explicit"
    return 0
  fi
  if have_tty; then
    if [[ "$action" == uninstall ]]; then
      if ask_choice "Remove" 3; then
        return 0
      fi
    else
      if ask_choice "Install" 1; then
        return 0
      fi
    fi
  fi
  # Notices go to stderr. The caller captures this function's stdout.
  if [[ "$action" == uninstall ]]; then
    printf '%s\n' "No terminal for a prompt. Removing both programs." >&2
    printf '%s\n' both
  else
    printf '%s\n' "No terminal for a prompt. Installing the terminal app." >&2
    printf '%s\n' tui
  fi
}

main() {
  local action=install
  local choice=""
  DO_BUILD=1
  BUILD_PROFILE="release"
  FROM_RELEASE=0
  FROM_SOURCE=0
  PRINT_TARGET=0

  while [ "$#" -gt 0 ]; do
    case "$1" in
      install) ;;
      uninstall | -uninstall | --uninstall) action=uninstall ;;
      --tui | --gui | --both)
        if [ -n "$choice" ]; then
          fail "pass only one of --tui, --gui, or --both"
        fi
        choice="${1#--}"
        ;;
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
  if [ -z "$choice" ] && [ "${INSTALL_GUI:-}" = "1" ]; then
    choice=both
  fi

  TARGET="$(detect_target)"
  if [ "$PRINT_TARGET" -eq 1 ]; then
    printf '%s\n' "$TARGET"
    exit 0
  fi

  print_mark

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

  local target
  target="$(choose_target "$action" "$choice")"

  if [ "$action" = uninstall ]; then
    uninstall_target "$target"
    return 0
  fi

  if [ "$FROM_SOURCE" -eq 1 ]; then
    install_from_source "${SCRIPT_DIR}" "$target"
  elif [ "$FROM_RELEASE" -eq 1 ]; then
    install_from_release "$target"
  elif [ -n "$SCRIPT_DIR" ] && command -v cargo >/dev/null 2>&1 && [ "$DO_BUILD" -eq 1 ]; then
    install_from_source "$SCRIPT_DIR" "$target"
  elif [ -n "$SCRIPT_DIR" ] && [ "$DO_BUILD" -eq 0 ]; then
    install_from_source "$SCRIPT_DIR" "$target"
  else
    install_from_release "$target"
  fi
}

# `curl | bash` reads this file from stdin. BASH_SOURCE is unset then, and
# `set -u` aborts on a bare ${BASH_SOURCE[0]}. An empty source means the
# file is being executed, not sourced.
if [[ -z "${BASH_SOURCE[0]:-}" || "${BASH_SOURCE[0]:-}" == "$0" ]]; then
  main "$@"
fi
