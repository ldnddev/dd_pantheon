use std::io::Write;
use std::process::{Command, Stdio};

/// Best-effort copy via `wl-copy` then `xclip`. Never a shell.
pub fn copy_text(text: &str) -> Result<(), String> {
    let candidates: &[(&str, &[&str])] =
        &[("wl-copy", &[]), ("xclip", &["-selection", "clipboard"])];
    let mut last_err: Option<String> = None;
    for (bin, args) in candidates {
        let path = match which::which(bin) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let mut child = match Command::new(&path)
            .args(*args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(err) => {
                last_err = Some(format!("{bin}: {err}"));
                continue;
            }
        };
        if let Some(mut stdin) = child.stdin.take() {
            if let Err(err) = stdin.write_all(text.as_bytes()) {
                last_err = Some(format!("{bin} stdin: {err}"));
                continue;
            }
        }
        match child.wait() {
            Ok(status) if status.success() => return Ok(()),
            Ok(status) => last_err = Some(format!("{bin} exited {status}")),
            Err(err) => last_err = Some(format!("{bin}: {err}")),
        }
    }
    Err(last_err.unwrap_or_else(|| "no wl-copy/xclip on PATH".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_helpers_error_mentions_path() {
        // We cannot force PATH empty without breaking which(git) elsewhere;
        // the error string is the contract when both are missing.
        if which::which("wl-copy").is_err() && which::which("xclip").is_err() {
            let err = copy_text("hello").expect_err("no helper");
            assert!(err.contains("wl-copy") || err.contains("xclip"), "{err}");
        }
    }
}
