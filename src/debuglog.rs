use crate::plan::StagedPlan;
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::sync::OnceLock;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;

static INIT: OnceLock<bool> = OnceLock::new();

pub fn log_path() -> PathBuf {
    dirs::state_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ldnddev/dd_pantheon/app.log")
}

/// Init file tracing when `debug_log = true` or `RUST_LOG` is set.
/// Never ANSI. Never called from the draw tick.
pub fn init(debug_log: bool) {
    let rust_log = std::env::var("RUST_LOG").ok().filter(|s| !s.is_empty());
    if !debug_log && rust_log.is_none() {
        return;
    }
    INIT.get_or_init(|| {
        let path = log_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let Ok(file) = OpenOptions::new().create(true).append(true).open(&path) else {
            return false;
        };
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
        let _ = fmt()
            .with_ansi(false)
            .with_writer(std::sync::Mutex::new(file))
            .with_env_filter(filter)
            .try_init();
        true
    });
}

pub fn plan_staged(plan: &StagedPlan) {
    if let Some(current) = plan.current() {
        tracing::info!(
            line = %current.redacted_shell_line(),
            safety = ?plan.safety(),
            "plan staged"
        );
    }
}
