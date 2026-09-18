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

Pin a version with `VERSION=v0.3.2`. Uninstall:

```sh
curl -fsSL https://raw.githubusercontent.com/ldnddev/dd_pantheon/main/install.sh | bash -s -- uninstall
```

From a clone, `./install.sh` builds with cargo when it is on `PATH`, otherwise
it uses the same GitHub package as curl:

```sh
./install.sh                 # cargo release build, or GitHub package
./install.sh --from-release  # always the GitHub package for this machine
./install.sh --from-source   # always cargo
./install.sh uninstall       # removes the binary + default theme, not app config
```

From a clone without installing:

```sh
cargo run -- --demo          # dummy sites, no spawn
cargo run                    # live Terminus session
cargo run -- --root ~/sites/acme-wp
```

**Setup and daily use:** [docs/tutorial.html](docs/tutorial.html)

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
