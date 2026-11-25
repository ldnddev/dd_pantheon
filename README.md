# dd_pantheon

A guided operations cockpit for Pantheon-hosted WordPress and Drupal sites. It never talks to the Pantheon HTTP API. Every remote or local action is a `terminus`, `lando`, or `git` subprocess, shown as a `CommandPlan` before anything is spawned.

Install the binary onto your PATH (default `~/.local/bin`). A curl one-liner
downloads the GitHub Release tarball for this OS/arch (Linux/macOS, x86_64 or
arm64). No Rust toolchain required:

```sh
curl -fsSL https://raw.githubusercontent.com/ldnddev/dd_pantheon/main/install.sh | bash
dd_pantheon --demo
dd_pantheon
dd_pantheon --root ~/sites/acme-wp
```

Pin a version with `VERSION=v0.3.8`. With a terminal to answer, the script
asks for the terminal app, the desktop window, or both. With no terminal it
installs the terminal app. `--tui`, `--gui`, and `--both` skip the question
(pass only one). `INSTALL_GUI=1` is the same as `--both`.

On Linux the install writes a launcher for each binary it installs.
`dd_pantheon.desktop` is `Terminal=true`. `dd_pantheon-gui.desktop` is
`Terminal=false`, `StartupNotify=true`, and
`StartupWMClass=dev.ldnddev.dd_pantheon`. `$XDG_DATA_HOME/applications` wins
when that variable is set. Otherwise a user install uses
`~/.local/share/applications`, and a prefix outside your home directory uses
`$PREFIX/share/applications`. macOS skips the launcher.

The window needs Vulkan (`libvulkan1` and a driver) and Wayland or X11.
Published release tarballs ship the terminal binary. `--gui` and `--both`
with `--from-release` fail and point at `--from-source`. A window-only
install does not write `dd_pantheon_theme.yml`. The theme file is copied
only when it is missing, and only for the terminal app or both.

Uninstall asks the same question (default both; with no terminal it removes
both). It removes only the chosen binary and its `.desktop` file. It leaves
`dd_pantheon_theme.yml` and `~/.config/ldnddev/dd_pantheon/` alone. Pass the
same `PREFIX`, `BINDIR`, and `XDG_DATA_HOME` you used to install.

```sh
curl -fsSL https://raw.githubusercontent.com/ldnddev/dd_pantheon/main/install.sh | bash -s -- uninstall
```

From a clone, `./install.sh` builds with cargo when it is on `PATH`, otherwise
it uses the same GitHub package as curl. The terminal build is the root
package. The window build is `cargo build --release -p dd_pantheon_gui`.

```sh
./install.sh                 # ask, then cargo release build or GitHub package
./install.sh --tui           # terminal only
./install.sh --gui           # window only
./install.sh --both          # both
./install.sh --from-release  # always the GitHub package for this machine
./install.sh --from-source   # always cargo
./install.sh uninstall       # chosen binary and launcher; leaves the theme file
```

From a clone without installing:

```sh
cargo run -- --demo          # dummy sites, no spawn
cargo run                    # live Terminus session
cargo run -- --root ~/sites/acme-wp
cargo run -p dd_pantheon_gui -- --demo
```

**Setup and daily use:** [tutorial](https://ldnddev.github.io/dd_pantheon/)

Quit is always `Ctrl+Q`. Bare `q` does not quit. Wipe has no `W` key.

## Refresh tutorial screenshots

Any change to demo fixtures, layout, chrome, or the modals shown in the tutorial
must recapture the PNGs. From the repo root:

```sh
cargo run --offline --example capture_demo -- docs/images
python3 docs/render_tui.py docs/images
```

Requires `rsvg-convert` (librsvg) for the PNG step. JSON/SVG intermediates are
gitignored; commit the updated `docs/images/demo-*.png` files. Full notes:
[docs/README.md](docs/README.md).

License: MIT
