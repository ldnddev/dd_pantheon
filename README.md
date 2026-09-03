# dd_pantheon

A guided operations cockpit for Pantheon-hosted WordPress and Drupal sites. It never talks to the Pantheon HTTP API. Every remote or local action is a `terminus`, `lando`, or `git` subprocess, shown as a `CommandPlan` before anything is spawned.

```sh
cargo run -- --demo          # layout lab, dummy sites, no spawn
cargo run                    # live Terminus session
cargo run -- --root ~/sites/acme-wp
```

**Setup and daily use:** [docs/tutorial.html](docs/tutorial.html)

Quit is always `Ctrl+Q`. Bare `q` does not quit. Wipe has no `W` key.

License: MIT
