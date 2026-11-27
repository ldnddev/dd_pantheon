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

Pin a version with `VERSION=v0.3.9`. With a terminal to answer, the script
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

The window binary `dd_pantheon-gui` boots the same terminal `App` and paints that state. It does not keep a second model of sites, plans, or jobs. Keys go to the terminal handler, with three window exceptions: F1, F2, Ctrl+/, and Ctrl+, open the Settings sheet (Themes, Window, Help) instead of the terminal settings overlay; `?` opens that sheet on Help; and when vim letters are off, `h` `j` `k` `l` `g` `G` do nothing on the panes (arrow keys still move). Inside a dialog those letters still go to the terminal handler. Density and the vim toggle live in `gui.json`, outside the site document. A modal owns the keyboard even if the sheet is open, and Ctrl+Q always does.

## Cockpit

Four panes under a header and a footer. The header shows the selected `site.env` and a busy mark while a job is live. The footer is pane-contextual. A wide tree footer reads `F1: Settings`, `F3: Doctor`, `j/k: Nav`, `Tab: Pane`, `Enter: Run`, `/: Filter`, `:: Palette`, `r: Refresh`, and `Ctrl+Q: Quit`. While a modal is up, the footer is `F1: Settings`, `Esc: Close`, `Ctrl+Q: Quit`, except LiveGate (below).

| Pane | What it shows |
|---|---|
| Tree | Title `Available sites (count)`, plus ` /query` and ` tag:name` when those filters are set. Each row is the site name, or the environment name, plus status: `frozen` on a site, connection mode and `lock` on an environment. Tags and framework stay in the inspector. A narrow pane ellipsizes the name and drops status only when one character of the name cannot fit. |
| Inspector | Framework, frozen, org, tag chips `[name]`, plan, upstream only when it is not the framework name, region, id, url (with `https://` when the value has no scheme), connection mode, environment lock, PHP, runtime, Drush, created, the latest backups, domains, HTTPS, HTTP basic auth on a second `lock` line, and the bound local app. Metrics sit beside that: period `d` / `w` / `M`, visitor / view / cache cards, and a visit sparkline. The period bar says the numbers are lagged. |
| Preview | Safety badge, target, redacted command, rationale, cwd, and workflow step. Buttons: Process, Copy, Cancel. |
| Log | Job output. Follows the tail until you scroll up. `G` or End follows again. Enter expands it to a full-size overlay. |

`Tab` / `Shift+Tab` cycle Tree, Inspector, Preview, Log. PageUp, PageDown, Home, and End work in the tree, inspector, log, help, and doctor. The terminal always treats `h` `j` `k` `l` `g` `G` as motion on the panes. The window does too, unless vim letters are off.

Mouse: click focuses a pane and selects a tree row. Click a metrics period. Click a tag chip to pin that tag filter; when the chip is selected, click `×` to stage removal. Click Process, Copy, or Cancel in the preview. Click the log once to focus it, again to expand it. The wheel scrolls the hovered pane. On the tree, the wheel moves the selection the same way `j` / `k` do. Clicking the dimmed chrome closes Doctor, Filter, Palette (list, not an open form), the error dialog, the log overlay, the tag picker, the org picker, backup pick, and domain remove. Confirm and LiveGate stay up.

Every remote or local action is staged as a `CommandPlan` before anything is spawned. `--demo` loads fixtures and never spawns. A `dry_run` plan never spawns. Read-only and mutating plans run after Process. Destructive plans ask for Enter or `y`. LiveGate asks you to type a word.

### Keys

Global, when no text field is open:

| Key | Action |
|---|---|
| F1, F2, Ctrl+/, Ctrl+, | Settings. F1 again closes and reverts the theme snapshot. In the window these open the Settings sheet and are not forwarded while no dialog is up. |
| `?` | Settings, Help page. In the window this opens the sheet on Help. |
| F3 | Doctor. Runs tool, catalog, and whoami jobs unless detection is skipped. F3, Esc, or `q` closes it. |
| Ctrl+Q | Quit. If a job is running, confirm first. Enter cancels jobs and quits. Esc stays. |
| Ctrl+C | Cancel the running job. |
| Ctrl+L | Login with a machine token. The token is masked. It is not written to config. It is visible to `ps` until the login process exits. Space toggles an env `TERMINUS_MACHINE_TOKEN` when one is set. |
| Ctrl+K or `:` | Command palette. |
| `/` | Filter sites by name or tag. Enter jumps to the highlighted hit. |
| Esc | Clear the filter and tag filter, or unstage the plan when the filter is already clear. |
| Ctrl+V | Paste into the focused text field in one insert. |

Tree and inspector, besides the motion keys:

| Key | Action |
|---|---|
| Enter | Tree: expand a site or keep the environment selected. Inspector: run the staged plan. |
| `h` / `l` or Left / Right | Tree: expand or collapse. Inspector: cycle tag chips. |
| `r` | Refresh inventory, metrics, and backups. Demo does not spawn. |
| `b` | Stage `backup:create` for the environment, then run (gates apply). |
| `e` | Deploy. Git-mode on dev stages the code path. `test` and `live` open a note form. `test` offers `--sync-content`. `--updatedb` is for Drupal. |
| `c` | Stage `env:clear-cache` from the tree or inspector. In the preview, `c` does nothing. |
| `s` / `S` | `lando start` / `lando stop` when a local app is bound. |
| `T` | Tag filter. Press again to clear. |
| `a` | Add a tag. |
| `x` | Remove the selected tag chip. |
| `n` | Create-site wizard: org, name, label, upstream, optional local bind. |
| `m` | CMS form. Never the metrics month. |
| `d` / `w` / Shift+M | Metrics period: day, week, month. |
| (no `W`) | Wipe is palette-only. It is backup-first. Live is LiveGate. |

Preview: Enter processes the command, `y` copies the redacted argv, Esc unstages when the filter is clear. Log: Enter or `e` expands, `g` jumps to the top and stops following, `G` follows the tail.

### Palette and forms

The palette lists the Terminus catalog from doctor, plus Lando. Type to filter, including a `lando` prefix. `lando init` is always available. Every other Lando command stays disabled until the selected site, or the working directory, has a Pantheon-recipe `.lando.yml`. Disabled rows do not open a form.

Enter on a row opens its argument form. Tab cycles fields. Space flips a toggle. Enter stages the plan and does not run it. You process it from the preview, where the safety gate still applies. The palette is how you reach domains, HTTPS, lock, backup restore, backup URL (`backup:get` prints a URL and does not download), clone content, multidev, wipe, and connection mode.

| Form | Notes |
|---|---|
| Login | Masked token. Env token toggle. |
| Confirm | Destructive. Enter or `y` runs. Esc cancels. |
| LiveGate | Command, target, and why sit above an input titled with the required word (`live`, or `database` for a Lando push that includes a database). The footer tells you to type that word and that Enter runs only when it matches. After a match it says `Enter: Run`. It does not say Enter confirms before the word matches. A mismatched prefix is an error. F1 and F2 do not open while the gate is collecting text. |
| Deploy note | Required on `test` and `live`. |
| CMS | Remote or local, `wp` or `drush`, command, history. Destructive Drush and WP verbs (`sql-drop`, `site-install`, and the same family) confirm. |
| Site create | Org, name, label, upstream, bind. |
| Multidev | Name up to 11 characters, lowercase letters, digits, and dashes. Not `dev`, `test`, or `live`. |
| Clone content | Origin, `--cc`, database only, files only, `--updatedb`. |
| Backup pick | Restore (destructive, live is LiveGate) or show the download URL. |
| Domain add / remove | |
| HTTPS | Certificate file paths, not file contents. |
| Lock | Username and a masked password. The password is redacted in the preview and the log. It is still visible to `ps` until `lock:enable` exits. |
| Tag add, tag picker, org picker | |
| Diffstat | Shown when a git connection change is blocked by uncommitted files. |
| Quit confirm | Only while a job is running. |

### Jobs, doctor, and redaction

Doctor prints Terminus, Lando, and git, whoami, the visible catalog count, and warnings. Versions and the catalog load through the job runner, not on the draw tick.

One mutating slot per target. At most four mutating jobs at once. Ctrl+C cancels. Output is held until a newline, redacted as a whole line, then walked again in finished JSON, including object keys, before the redact rules are dropped. A secret that itself contains a newline can still split across log lines. The machine token and the lock password are passed as argv, so they show up in the process list until the child exits.

The idle loop redraws only when something changed. Jobs wake about every 50ms and the loader every 150ms. Mouse-move and key-up are ignored.

### Themes

Both binaries load `dd_pantheon_theme.yml` in the same order and resolve one palette. The terminal settings overlay and the window Settings sheet open on Themes. Esc in the color editor steps back to the list, then closes and reverts the snapshot taken when it opened. `t` switches the save target. `y` saves. The window sheet adds a Window page (density, vim letters) and a Help page. The first help row is `Settings · F1 · F1`. The terminal Help page is the same overlay's help text, not that sheet.

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
