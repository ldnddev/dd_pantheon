# Tutorial assets

Operator guide: [index.html](index.html) (GitHub Pages:
https://ldnddev.github.io/dd_pantheon/). `tutorial.html` redirects there so
older links still work.

GitHub Pages publishes this folder from `main` via `.github/workflows/pages.yml`.
`.nojekyll` is here so a branch `/docs` source would also serve the HTML as-is
instead of running Jekyll.

## Refresh UI shots after a change

The PNGs in `docs/images/` are captured from the **real `--demo` TUI** (Ratatui
`TestBackend`), not drawn by hand. Recapture whenever you change:

- demo fixtures (`src/fixtures.rs`)
- chrome (header, footer, pane titles, DEMO badge)
- layouts (`src/ui/layouts.rs` and the widgets they host)
- any modal in the shot list (Help, palette, CMS, deploy note, LiveGate)
- theme tokens that those screens use

From the **repo root**:

```sh
cargo run --offline --example capture_demo -- docs/images
python3 docs/render_tui.py docs/images
```

1. `examples/capture_demo.rs` drives `App::new_demo_in`, sends the keys for each
   frame, and writes `docs/images/demo-*.json`.
2. `docs/render_tui.py` turns those JSON grids into SVG, then PNG via
   `rsvg-convert` (librsvg). Install that package if the second command fails.

Do **not** commit `*.json` or `*.svg` — they are gitignored. Commit the PNGs.

| File | How it is produced |
|---|---|
| `demo-cockpit.png` | default `--demo` (acme-wp.test selected) |
| `demo-help.png` | `F1` |
| `demo-palette.png` | `:` then type `wipe` |
| `demo-cms.png` | `m` |
| `demo-deploy.png` | `e` on test (note form) |
| `demo-livegate.png` | select live, `e`, Enter (demo flag off only so LiveGate opens; still fixtures) |

If you add a screenshot, add a scene in `capture_demo.rs`, a row here, and a
`<figure>` in `index.html`.
