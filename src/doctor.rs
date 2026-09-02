use crate::catalog::{
    CatalogEntry, merge_pantheon_recipe_extras, parse_lando_help, parse_terminus_list,
};
use crate::tools::{Toolset, detect_tools, run_output};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthState {
    Unknown,
    LoggedOut,
    LoggedIn { email: String, id: Option<String> },
}

#[derive(Clone, Debug)]
pub struct DoctorReport {
    pub tools: Toolset,
    pub catalog: Vec<CatalogEntry>,
    pub auth: AuthState,
    pub warnings: Vec<String>,
}

impl DoctorReport {
    pub fn render(&self) -> String {
        render(&self.tools, &self.auth, &self.catalog, &self.warnings)
    }
}

pub fn render(
    tools: &Toolset,
    auth: &AuthState,
    catalog: &[CatalogEntry],
    warnings: &[String],
) -> String {
    let mut lines = Vec::new();
    lines.push("Doctor".into());
    lines.push(String::new());
    lines.push("Tools".into());
    lines.push(tool_line("terminus", &tools.terminus));
    lines.push(tool_line("lando", &tools.lando));
    lines.push(tool_line("git", &tools.git));
    lines.push(String::new());
    lines.push("Auth".into());
    lines.push(match auth {
        AuthState::Unknown => "  whoami        not checked".into(),
        AuthState::LoggedOut => "  whoami        not logged in".into(),
        AuthState::LoggedIn { email, .. } => format!("  whoami        {email}"),
    });
    let visible = catalog
        .iter()
        .filter(|e| e.kind != crate::catalog::CatalogKind::Hidden)
        .count();
    lines.push(String::new());
    lines.push("Catalog".into());
    lines.push(format!(
        "  commands      {visible} visible ({} total)",
        catalog.len()
    ));
    if !warnings.is_empty() {
        lines.push(String::new());
        lines.push("Warnings".into());
        for w in warnings {
            lines.push(format!("  {w}"));
        }
    }
    lines.push(String::new());
    lines.push("PR 2: detect uses blocking Command::output at launch, not the draw tick.".into());
    lines.push("Job runner / login modal land in PR 3. Enter still does not spawn.".into());
    lines.join("\n")
}

fn tool_line(name: &str, bin: &Option<crate::tools::ToolBinary>) -> String {
    match bin {
        Some(b) => {
            let ver = b.version.as_deref().unwrap_or("?");
            format!("  {name:<12}  ok  {ver}  {}", b.path.display())
        }
        None => format!("  {name:<12}  MISSING"),
    }
}

/// Blocking detect + catalog + whoami. Call from App::new (or F3 refresh), never draw.
pub fn run_doctor(include_in_app_lando: bool, local_cwd: Option<&Path>) -> DoctorReport {
    let mut report = DoctorReport {
        tools: detect_tools(),
        catalog: Vec::new(),
        auth: AuthState::Unknown,
        warnings: Vec::new(),
    };

    if let Some(term) = &report.tools.terminus {
        match run_output(&term.path, &["list", "--format=json"], None) {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                match parse_terminus_list(&stdout) {
                    Ok(entries) => report.catalog.extend(entries),
                    Err(err) => report
                        .warnings
                        .push(format!("terminus list JSON parse failed: {err:#}")),
                }
            }
            Err(err) => report
                .warnings
                .push(format!("terminus list failed: {err:#}")),
        }
        report.auth = whoami(&term.path);
    } else {
        report
            .warnings
            .push("terminus not on PATH — inventory, login, and palette disabled".into());
    }

    if let Some(lando) = &report.tools.lando {
        match run_output(&lando.path, &["--help"], None) {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                report.catalog.extend(parse_lando_help(&stdout));
            }
            Err(err) => report
                .warnings
                .push(format!("lando --help failed: {err:#}")),
        }
        if include_in_app_lando {
            if let Some(cwd) = local_cwd {
                match run_output(&lando.path, &["--help"], Some(cwd)) {
                    Ok(out) => {
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        for e in parse_lando_help(&stdout) {
                            if !report.catalog.iter().any(|c| c.name == e.name) {
                                report.catalog.push(e);
                            }
                        }
                    }
                    Err(_) => merge_pantheon_recipe_extras(&mut report.catalog),
                }
            } else {
                merge_pantheon_recipe_extras(&mut report.catalog);
            }
        }
    } else {
        report
            .warnings
            .push("lando not on PATH — local workflows disabled".into());
    }

    if report.tools.git.is_none() {
        report
            .warnings
            .push("git not on PATH — git-mode deploy push disabled".into());
    }

    report
}

fn whoami(terminus: &Path) -> AuthState {
    match run_output(terminus, &["auth:whoami", "--format=json"], None) {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&out.stderr);
            let logged_out = stdout.is_empty()
                && (out.status.success() || stderr.to_ascii_lowercase().contains("not logged in"));
            if logged_out {
                return AuthState::LoggedOut;
            }
            if stdout.is_empty() {
                return AuthState::LoggedOut;
            }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&stdout) {
                let email = val
                    .get("email")
                    .and_then(|v| v.as_str())
                    .or_else(|| val.as_str())
                    .unwrap_or(stdout.as_str())
                    .to_string();
                let id = val
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                return AuthState::LoggedIn { email, id };
            }
            AuthState::LoggedIn {
                email: stdout.trim_matches('"').to_string(),
                id: None,
            }
        }
        Err(_) => AuthState::Unknown,
    }
}
