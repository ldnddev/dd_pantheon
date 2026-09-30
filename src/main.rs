use anyhow::Result;
use crossterm::{
    ExecutableCommand,
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use dd_pantheon::app::{App, LaunchOpts};
use dd_pantheon::config::default_config_dir;
use dd_pantheon::input::event_is_actionable;
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io::{self, Write, stdout};
use std::path::PathBuf;
use std::time::Duration;

fn main() -> Result<()> {
    match parse_cli(std::env::args().skip(1)) {
        Ok(CliAction::PrintHelp) => {
            print_help(&mut io::stdout())?;
            return Ok(());
        }
        Err(err) => {
            let mut stderr = io::stderr();
            writeln!(stderr, "Error: {err}")?;
            writeln!(stderr)?;
            print_help(&mut stderr)?;
            std::process::exit(2);
        }
        Ok(CliAction::Run(opts)) => run_app(opts)?,
    }
    Ok(())
}

fn run_app(cli: RunOpts) -> Result<()> {
    // Detect/catalog run here, before raw mode, so Command::output is not on the draw tick.
    let mut app = App::new(LaunchOpts {
        demo: cli.demo,
        root: cli.root,
        project_root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        config_dir: default_config_dir(),
        skip_detect: false,
    })?;

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let _ = stdout().execute(EnableMouseCapture);

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    if app.state.demo {
        app.state.show_toast(
            dd_pantheon::toast::ToastLevel::Info,
            "demo: fixtures on, user plans will not spawn",
        );
    }

    let result = (|| -> Result<()> {
        let mut needs_draw = true;
        loop {
            if app.tick() {
                needs_draw = true;
            }
            if needs_draw {
                terminal.draw(|f| app.draw(f))?;
                needs_draw = false;
            }
            if app.state.should_quit {
                break;
            }
            // Idle: wait up to a minute for input. Jobs/toasts/debounces wake sooner.
            let timeout = app.next_poll_timeout().unwrap_or(Duration::from_secs(60));
            if !event::poll(timeout)? {
                continue;
            }
            loop {
                match event::read()? {
                    ev if !event_is_actionable(&ev) => {}
                    Event::Key(key) => {
                        if app.handle_key(key)? {
                            app.state.should_quit = true;
                        }
                        needs_draw = true;
                    }
                    Event::Mouse(mouse) => {
                        if app.handle_mouse(mouse)? {
                            app.state.should_quit = true;
                        }
                        needs_draw = true;
                    }
                    Event::Resize(_, _) => needs_draw = true,
                    _ => {}
                }
                if app.state.should_quit || !event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }
        Ok(())
    })();

    let _ = app.save();
    let _ = stdout().execute(DisableMouseCapture);
    let _ = stdout().execute(LeaveAlternateScreen);
    let _ = disable_raw_mode();
    result
}

fn print_help(w: &mut impl Write) -> Result<()> {
    writeln!(w, "dd_pantheon — guided operations cockpit for Pantheon")?;
    writeln!(w)?;
    writeln!(w, "Usage:")?;
    writeln!(w, "  dd_pantheon [--demo] [--root <path>]")?;
    writeln!(w)?;
    writeln!(w, "Options:")?;
    writeln!(
        w,
        "  --demo          Layout lab with dummy sites (no Terminus)"
    )?;
    writeln!(w, "  --root <path>   Bind a local project path")?;
    writeln!(w, "  -h, --help      Show this help")?;
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum CliAction {
    Run(RunOpts),
    PrintHelp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RunOpts {
    demo: bool,
    root: Option<PathBuf>,
}

fn parse_cli<I>(args: I) -> std::result::Result<CliAction, String>
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
mod tests {
    use super::*;

    #[test]
    fn parse_demo_and_help() {
        let cli = parse_cli(vec!["--demo".to_string()]).unwrap();
        assert_eq!(
            cli,
            CliAction::Run(RunOpts {
                demo: true,
                root: None
            })
        );
        assert_eq!(
            parse_cli(vec!["--help".to_string()]).unwrap(),
            CliAction::PrintHelp
        );
    }
}
