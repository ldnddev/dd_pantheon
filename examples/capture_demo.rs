//! Render `--demo` frames to JSON cell grids for docs screenshots.
//!
//! ```sh
//! cargo run --offline --example capture_demo -- docs/images
//! python3 docs/render_tui.py docs/images
//! ```

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dd_pantheon::app::App;
use dd_pantheon::state::TreeSel;
use ratatui::backend::TestBackend;
use ratatui::style::{Color, Modifier};
use ratatui::Terminal;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

const COLS: u16 = 112;
const ROWS: u16 = 34;

#[derive(Serialize)]
struct FrameDump {
    name: String,
    width: u16,
    height: u16,
    cells: Vec<CellDump>,
}

#[derive(Serialize)]
struct CellDump {
    x: u16,
    y: u16,
    ch: String,
    fg: String,
    bg: String,
    bold: bool,
}

fn main() -> Result<()> {
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "docs/images".into()),
    );
    fs::create_dir_all(&out)?;
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cfg = tempfile_dir()?;

    dump(&out, "demo-cockpit", demo_app(&crate_root, &cfg)?, &[])?;
    dump(
        &out,
        "demo-help",
        demo_app(&crate_root, &cfg)?,
        &[key(KeyCode::F(1))],
    )?;
    dump(
        &out,
        "demo-palette",
        demo_app(&crate_root, &cfg)?,
        &[
            key(KeyCode::Char(':')),
            key(KeyCode::Char('w')),
            key(KeyCode::Char('i')),
            key(KeyCode::Char('p')),
            key(KeyCode::Char('e')),
        ],
    )?;
    dump(
        &out,
        "demo-cms",
        demo_app(&crate_root, &cfg)?,
        &[key(KeyCode::Char('m'))],
    )?;
    dump(
        &out,
        "demo-deploy",
        demo_app(&crate_root, &cfg)?,
        &[key(KeyCode::Char('e'))],
    )?;

    let mut live = demo_app(&crate_root, &cfg)?;
    live.state.demo = false;
    live.state.selected = TreeSel::Env {
        site: "acme-wp".into(),
        env: "live".into(),
    };
    dump(
        &out,
        "demo-livegate",
        live,
        &[key(KeyCode::Char('e')), key(KeyCode::Enter)],
    )?;

    eprintln!("wrote JSON frames under {}", out.display());
    Ok(())
}

fn demo_app(root: &Path, cfg: &Path) -> Result<App> {
    App::new_demo_in(root, cfg)
}

fn dump(dir: &Path, name: &str, mut app: App, keys: &[KeyEvent]) -> Result<()> {
    for k in keys {
        let _ = app.handle_key(*k)?;
    }
    let backend = TestBackend::new(COLS, ROWS);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|f| app.draw(f))?;
    let buf = terminal.backend().buffer();
    let mut cells = Vec::with_capacity((COLS * ROWS) as usize);
    for y in 0..ROWS {
        for x in 0..COLS {
            let cell = &buf[(x, y)];
            cells.push(CellDump {
                x,
                y,
                ch: cell.symbol().to_string(),
                fg: color_hex(cell.fg, (0xf5, 0xf6, 0xf7)),
                bg: color_hex(cell.bg, (0x2a, 0x2d, 0x31)),
                bold: cell.modifier.contains(Modifier::BOLD),
            });
        }
    }
    let dump = FrameDump {
        name: name.into(),
        width: COLS,
        height: ROWS,
        cells,
    };
    let path = dir.join(format!("{name}.json"));
    fs::write(&path, serde_json::to_string(&dump)?)?;
    eprintln!("  {}", path.display());
    Ok(())
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn color_hex(c: Color, fallback: (u8, u8, u8)) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("#{r:02X}{g:02X}{b:02X}"),
        Color::Black => "#000000".into(),
        Color::White => "#FFFFFF".into(),
        Color::Gray => "#9EA3AA".into(),
        Color::DarkGray => "#2A2D31".into(),
        Color::Red => "#E57373".into(),
        Color::Green => "#82E0AA".into(),
        Color::Yellow => "#F5C469".into(),
        Color::Blue => "#64B4F5".into(),
        Color::Magenta => "#C39BD3".into(),
        Color::Cyan => "#5DADE2".into(),
        Color::LightRed => "#E57373".into(),
        Color::LightGreen => "#82E0AA".into(),
        Color::LightYellow => "#F5C469".into(),
        Color::LightBlue => "#64B4F5".into(),
        Color::LightMagenta => "#C39BD3".into(),
        Color::LightCyan => "#5DADE2".into(),
        _ => format!("#{:02X}{:02X}{:02X}", fallback.0, fallback.1, fallback.2),
    }
}

fn tempfile_dir() -> Result<PathBuf> {
    let p = std::env::temp_dir().join(format!(
        "dd_pantheon_capture_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir_all(&p)?;
    Ok(p)
}
