//! dd_pantheon window. The terminal crate stays the source of behavior.

mod prefs;
mod shell;
mod shortcuts;
mod windows;

#[cfg(test)]
mod tests;

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use gpui::{App, AppContext, Focusable};
use gpui_component::Root;
use shell::{Boot, Shell};

fn main() -> ExitCode {
    match parse_cli(std::env::args().skip(1)) {
        Ok(CliAction::PrintHelp) => {
            print_help(&mut io::stdout());
            ExitCode::SUCCESS
        }
        Err(err) => {
            let mut stderr = io::stderr();
            let _ = writeln!(stderr, "Error: {err}");
            let _ = writeln!(stderr);
            print_help(&mut stderr);
            ExitCode::from(2)
        }
        Ok(CliAction::Run(opts)) => {
            run(Boot::from_run(opts));
            ExitCode::SUCCESS
        }
    }
}

fn run(boot: Boot) {
    gpui_platform::application().run(move |cx: &mut App| {
        gpui_component::init(cx);
        let boot = boot.clone();
        cx.spawn(async move |cx| {
            cx.update(|cx| {
                let bounds = windows::open_bounds(cx);
                cx.open_window(windows::main_window_options(bounds), |window, cx| {
                    let view = cx.new(|cx| {
                        let shell = Shell::open(boot, cx);
                        window.focus(&shell.focus_handle(cx), cx);
                        shell
                    });
                    cx.new(|cx| Root::new(view, window, cx))
                })
                .expect("open window");
                cx.activate(true);
            });
        })
        .detach();
    });
}

pub fn print_help(w: &mut impl Write) {
    let _ = writeln!(w, "dd_pantheon — guided operations cockpit for Pantheon");
    let _ = writeln!(w);
    let _ = writeln!(w, "Usage:");
    let _ = writeln!(w, "  dd_pantheon-gui [--demo] [--root <path>]");
    let _ = writeln!(w);
    let _ = writeln!(w, "Options:");
    let _ = writeln!(
        w,
        "  --demo          Layout lab with dummy sites (no Terminus plans)"
    );
    let _ = writeln!(w, "  --root <path>   Bind a local project path");
    let _ = writeln!(w, "  -h, --help      Show this help");
}

#[derive(Debug, PartialEq, Eq)]
enum CliAction {
    Run(RunOpts),
    PrintHelp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOpts {
    pub demo: bool,
    pub root: Option<PathBuf>,
}

fn parse_cli<I>(args: I) -> Result<CliAction, String>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let mut demo = false;
    let mut root = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(CliAction::PrintHelp),
            "--demo" => demo = true,
            "--root" => {
                let value = args.next().ok_or("Missing value for --root")?;
                root = Some(PathBuf::from(value));
            }
            other if other.starts_with("--root=") => {
                root = Some(PathBuf::from(&other["--root=".len()..]));
            }
            other if other.starts_with('-') => {
                return Err(format!("Unknown option: {other}"));
            }
            other => return Err(format!("Unexpected argument: {other}")),
        }
    }
    Ok(CliAction::Run(RunOpts { demo, root }))
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    #[test]
    fn help_parses_without_opening_a_window() {
        assert_eq!(
            parse_cli(vec!["--help".to_string()]).unwrap(),
            CliAction::PrintHelp
        );
        assert_eq!(
            parse_cli(vec!["-h".to_string()]).unwrap(),
            CliAction::PrintHelp
        );
        let mut buf = Vec::new();
        print_help(&mut buf);
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("--demo"));
        assert!(text.contains("--root"));
        assert!(!text.contains("settings-panel"));
    }

    #[test]
    fn demo_and_root_match_the_terminal_flags() {
        let cli = parse_cli(["--demo".to_string(), "--root".to_string(), "/tmp/site".to_string()])
            .unwrap();
        assert_eq!(
            cli,
            CliAction::Run(RunOpts {
                demo: true,
                root: Some(PathBuf::from("/tmp/site")),
            })
        );
        let eq = parse_cli(["--root=/tmp/other".to_string()]).unwrap();
        assert_eq!(
            eq,
            CliAction::Run(RunOpts {
                demo: false,
                root: Some(PathBuf::from("/tmp/other")),
            })
        );
    }
}
