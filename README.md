# dd_pantheon

A guided operations cockpit for Pantheon-hosted WordPress and Drupal sites. It never talks to the Pantheon HTTP API. Every remote or local action is a `terminus`, `lando`, or `git` subprocess, shown as a `CommandPlan` before anything is spawned.

```sh
cargo run -- --demo          # layout lab, dummy sites, no spawn
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
