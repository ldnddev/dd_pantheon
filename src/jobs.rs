use crate::plan::{CommandPlan, ToolKind};
use anyhow::{Result, anyhow};
use std::collections::VecDeque;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

pub const CHANNEL_CAP: usize = 256;
pub const LOG_MAX_LINES: usize = 4000;
pub const LOG_MAX_BYTES: usize = 1024 * 1024;
const CHUNK: usize = 4096;
const KILL_GRACE: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct JobId(pub Uuid);

impl JobId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JobKind {
    User,
    Workflow {
        step: usize,
        total: usize,
    },
    DoctorVersion {
        tool: ToolKind,
    },
    DoctorList,
    DoctorWhoami,
    SiteList,
    EnvList {
        site: String,
    },
    EnvInfo {
        site: String,
        env: String,
    },
    OrgList {
        site: String,
    },
    TagList {
        site: String,
        org: String,
    },
    Metrics {
        site: String,
        env: String,
        period: String,
    },
}

impl JobKind {
    pub fn quiet(self: &Self) -> bool {
        matches!(
            self,
            Self::DoctorVersion { .. }
                | Self::DoctorList
                | Self::DoctorWhoami
                | Self::SiteList
                | Self::EnvList { .. }
                | Self::EnvInfo { .. }
                | Self::OrgList { .. }
                | Self::TagList { .. }
                | Self::Metrics { .. }
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JobStatus {
    Queued,
    Running { pid: u32, pgid: i32 },
    Succeeded { exit: i32 },
    Failed { exit: Option<i32>, err: String },
    Cancelled,
    TimedOut,
}

impl JobStatus {
    pub fn is_live(&self) -> bool {
        matches!(self, Self::Queued | Self::Running { .. })
    }
}

#[derive(Clone, Debug)]
pub struct LogBuffer {
    pub lines: VecDeque<String>,
    bytes: usize,
    current: String,
}

impl Default for LogBuffer {
    fn default() -> Self {
        Self {
            lines: VecDeque::new(),
            bytes: 0,
            current: String::new(),
        }
    }
}

impl LogBuffer {
    pub fn push(&mut self, chunk: &str) {
        self.current.push_str(chunk);
        while let Some(i) = self.current.find('\n') {
            let mut line: String = self.current.drain(..=i).collect();
            if line.ends_with('\n') {
                line.pop();
                if line.ends_with('\r') {
                    line.pop();
                }
            }
            self.push_line(line);
        }
    }

    fn push_line(&mut self, line: String) {
        self.bytes = self.bytes.saturating_add(line.len());
        self.lines.push_back(line);
        while self.lines.len() > LOG_MAX_LINES || self.bytes > LOG_MAX_BYTES {
            if let Some(old) = self.lines.pop_front() {
                self.bytes = self.bytes.saturating_sub(old.len());
            } else {
                break;
            }
        }
    }

    pub fn text(&self) -> String {
        let mut s = self.lines.iter().cloned().collect::<Vec<_>>().join("\n");
        if !self.current.is_empty() {
            if !s.is_empty() {
                s.push('\n');
            }
            s.push_str(&self.current);
        }
        s
    }
}

#[derive(Debug)]
pub struct Job {
    pub id: JobId,
    pub kind: JobKind,
    pub plan: CommandPlan,
    pub status: JobStatus,
    pub started_at: Instant,
    pub log: LogBuffer,
    pub stdout_raw: String,
    pub json: Option<serde_json::Value>,
    pub cancel: Arc<AtomicBool>,
    pub pgid: Arc<AtomicI32>,
}

#[derive(Debug)]
pub enum JobEvent {
    Started {
        id: JobId,
        pid: u32,
        pgid: i32,
    },
    Chunk {
        id: JobId,
        stream: Stream,
        text: String,
    },
    Exited {
        id: JobId,
        status: JobStatus,
        stdout_raw: String,
    },
}

pub struct JobHub {
    pub tx: SyncSender<JobEvent>,
    pub rx: Receiver<JobEvent>,
}

impl JobHub {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::sync_channel(CHANNEL_CAP);
        Self { tx, rx }
    }
}

/// Build the child `Command` (execvp-style). Used by spawn and tests.
pub fn build_command(plan: &CommandPlan) -> Command {
    let mut cmd = Command::new(&plan.binary);
    cmd.args(plan.argv_with_injection(true))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = &plan.cwd {
        cmd.current_dir(cwd);
    }
    for (k, v) in &plan.extra_env {
        cmd.env(k, v);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd
}

pub fn spawn(tx: SyncSender<JobEvent>, plan: CommandPlan, kind: JobKind) -> Result<Job> {
    if plan.dry_run {
        return Err(anyhow!("dry_run: refuse to spawn"));
    }
    let id = JobId::new();
    let cancel = Arc::new(AtomicBool::new(false));
    let pgid = Arc::new(AtomicI32::new(0));
    let job = Job {
        id,
        kind: kind.clone(),
        plan: plan.clone(),
        status: JobStatus::Queued,
        started_at: Instant::now(),
        log: LogBuffer::default(),
        stdout_raw: String::new(),
        json: None,
        cancel: cancel.clone(),
        pgid: pgid.clone(),
    };
    thread::Builder::new()
        .name(format!("job-{}", id.0))
        .spawn(move || run_job(tx, id, plan, cancel, pgid))
        .map_err(|e| anyhow!("spawn thread: {e}"))?;
    Ok(job)
}

pub fn request_cancel(job: &Job) {
    job.cancel.store(true, Ordering::SeqCst);
    let pgid = job.pgid.load(Ordering::SeqCst);
    if pgid != 0 {
        kill_group(pgid, libc::SIGTERM);
    }
}

fn run_job(
    tx: SyncSender<JobEvent>,
    id: JobId,
    plan: CommandPlan,
    cancel: Arc<AtomicBool>,
    pgid_slot: Arc<AtomicI32>,
) {
    let mut cmd = build_command(&plan);
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(err) => {
            let _ = tx.send(JobEvent::Exited {
                id,
                status: JobStatus::Failed {
                    exit: None,
                    err: err.to_string(),
                },
                stdout_raw: String::new(),
            });
            return;
        }
    };
    let pid = child.id();
    let pgid = pid as i32;
    pgid_slot.store(pgid, Ordering::SeqCst);
    let _ = tx.send(JobEvent::Started { id, pid, pgid });

    let omitted = Arc::new(AtomicUsize::new(0));
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let tx_out = tx.clone();
    let omit_out = omitted.clone();
    let out_h = thread::spawn(move || {
        if let Some(r) = stdout {
            read_stream(tx_out, id, Stream::Stdout, r, omit_out);
        }
    });
    let tx_err = tx.clone();
    let omit_err = omitted.clone();
    let err_h = thread::spawn(move || {
        if let Some(r) = stderr {
            read_stream(tx_err, id, Stream::Stderr, r, omit_err);
        }
    });

    let timeout = plan.timeout;
    let started = Instant::now();
    let mut sent_term = false;
    let mut term_at = None;
    let mut timed_out = false;
    let mut cancelled = false;
    let status = loop {
        if cancel.load(Ordering::SeqCst) && !sent_term {
            kill_group(pgid, libc::SIGTERM);
            sent_term = true;
            term_at = Some(Instant::now());
            cancelled = true;
        }
        if let Some(limit) = timeout {
            if started.elapsed() >= limit && !sent_term {
                kill_group(pgid, libc::SIGTERM);
                sent_term = true;
                term_at = Some(Instant::now());
                timed_out = true;
            }
        }
        if let Some(t) = term_at {
            if t.elapsed() >= KILL_GRACE {
                kill_group(pgid, libc::SIGKILL);
            }
        }
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(err) => {
                let _ = out_h.join();
                let _ = err_h.join();
                flush_omitted(&tx, id, &omitted);
                let _ = tx.send(JobEvent::Exited {
                    id,
                    status: JobStatus::Failed {
                        exit: None,
                        err: err.to_string(),
                    },
                    stdout_raw: String::new(),
                });
                return;
            }
        }
    };
    let _ = out_h.join();
    let _ = err_h.join();
    flush_omitted(&tx, id, &omitted);

    let exit = status.code().unwrap_or(-1);
    let job_status = if cancelled {
        JobStatus::Cancelled
    } else if timed_out {
        JobStatus::TimedOut
    } else if status.success() {
        JobStatus::Succeeded { exit }
    } else {
        JobStatus::Failed {
            exit: Some(exit),
            err: format!("exit {exit}"),
        }
    };
    let _ = tx.send(JobEvent::Exited {
        id,
        status: job_status,
        stdout_raw: String::new(),
    });
}

fn read_stream<R: Read>(
    tx: SyncSender<JobEvent>,
    id: JobId,
    stream: Stream,
    mut reader: R,
    omitted: Arc<AtomicUsize>,
) {
    let mut buf = [0u8; CHUNK];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let text = String::from_utf8_lossy(&buf[..n]).into_owned();
                send_chunk(&tx, id, stream, text, &omitted);
            }
            Err(_) => break,
        }
    }
}

fn send_chunk(
    tx: &SyncSender<JobEvent>,
    id: JobId,
    stream: Stream,
    mut text: String,
    omitted: &AtomicUsize,
) {
    let n = omitted.swap(0, Ordering::SeqCst);
    if n > 0 {
        text = format!("… omitted {n} bytes …\n{text}");
    }
    match tx.try_send(JobEvent::Chunk { id, stream, text }) {
        Ok(()) => {}
        Err(TrySendError::Full(JobEvent::Chunk { text, .. })) => {
            omitted.fetch_add(text.len(), Ordering::SeqCst);
        }
        Err(TrySendError::Full(_)) => {
            omitted.fetch_add(1, Ordering::SeqCst);
        }
        Err(TrySendError::Disconnected(_)) => {}
    }
}

fn flush_omitted(tx: &SyncSender<JobEvent>, id: JobId, omitted: &AtomicUsize) {
    let n = omitted.swap(0, Ordering::SeqCst);
    if n == 0 {
        return;
    }
    let _ = tx.send(JobEvent::Chunk {
        id,
        stream: Stream::Stderr,
        text: format!("… omitted {n} bytes …\n"),
    });
}

fn kill_group(pgid: i32, sig: i32) {
    if pgid <= 0 {
        return;
    }
    #[cfg(unix)]
    unsafe {
        libc::killpg(pgid, sig);
    }
}

/// Try-send helper used by the omit-bytes test.
pub fn try_send_chunk(
    tx: &SyncSender<JobEvent>,
    id: JobId,
    text: String,
    omitted: &AtomicUsize,
) -> Result<(), TrySendError<JobEvent>> {
    let n = omitted.load(Ordering::SeqCst);
    let mut text = text;
    if n > 0 {
        let n = omitted.swap(0, Ordering::SeqCst);
        text = format!("… omitted {n} bytes …\n{text}");
    }
    match tx.try_send(JobEvent::Chunk {
        id,
        stream: Stream::Stdout,
        text,
    }) {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(ev)) => {
            if let JobEvent::Chunk { text, .. } = &ev {
                omitted.fetch_add(text.len(), Ordering::SeqCst);
            }
            Err(TrySendError::Full(ev))
        }
        Err(e) => Err(e),
    }
}

pub fn drain_timeout(rx: &Receiver<JobEvent>, timeout: Duration) -> Vec<JobEvent> {
    let deadline = Instant::now() + timeout;
    let mut out = Vec::new();
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        match rx.recv_timeout(left.min(Duration::from_millis(50))) {
            Ok(ev) => {
                let done = matches!(ev, JobEvent::Exited { .. });
                out.push(ev);
                if done {
                    break;
                }
            }
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{PlanTarget, SafetyTier};
    use std::path::PathBuf;

    fn raw_plan(bin: &str, argv: &[&str]) -> CommandPlan {
        CommandPlan {
            tool: ToolKind::Git,
            binary: PathBuf::from(bin),
            argv: argv.iter().map(|s| s.to_string()).collect(),
            cwd: None,
            why: "test".into(),
            safety: SafetyTier::ReadOnly,
            target: PlanTarget::None,
            dry_run: false,
            timeout: Some(Duration::from_secs(5)),
            expects_json: false,
            extra_env: vec![],
            redact: vec![],
            confirm_with_yes: false,
        }
    }

    #[test]
    fn echo_job_succeeds() {
        let hub = JobHub::new();
        let plan = raw_plan("/bin/echo", &["hello-from-job"]);
        let job = spawn(hub.tx.clone(), plan, JobKind::User).expect("spawn");
        let events = drain_timeout(&hub.rx, Duration::from_secs(3));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, JobEvent::Started { id, .. } if *id == job.id))
        );
        let stdout: String = events
            .iter()
            .filter_map(|e| match e {
                JobEvent::Chunk {
                    stream: Stream::Stdout,
                    text,
                    ..
                } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(stdout.contains("hello-from-job"), "{stdout}");
        assert!(events.iter().any(|e| matches!(
            e,
            JobEvent::Exited {
                status: JobStatus::Succeeded { .. },
                ..
            }
        )));
    }

    #[test]
    fn sleep_job_cancels() {
        let hub = JobHub::new();
        let mut plan = raw_plan("/bin/sleep", &["30"]);
        plan.timeout = Some(Duration::from_secs(10));
        let job = spawn(hub.tx.clone(), plan, JobKind::User).expect("spawn");
        // wait for Started
        let start = Instant::now();
        loop {
            match hub.rx.recv_timeout(Duration::from_secs(2)) {
                Ok(JobEvent::Started { .. }) => break,
                Ok(_) => {
                    if start.elapsed() > Duration::from_secs(2) {
                        panic!("no Started");
                    }
                }
                Err(_) => panic!("timeout waiting Started"),
            }
        }
        request_cancel(&job);
        let events = drain_timeout(&hub.rx, Duration::from_secs(5));
        assert!(
            events.iter().any(|e| matches!(
                e,
                JobEvent::Exited {
                    status: JobStatus::Cancelled | JobStatus::TimedOut,
                    ..
                }
            )),
            "{events:?}"
        );
    }

    #[test]
    fn login_command_puts_token_in_argv_not_env() {
        let plan =
            crate::workflows::auth::plan_login(PathBuf::from("terminus"), "super-secret-token");
        assert!(
            plan.argv
                .iter()
                .any(|a| a == "--machine-token=super-secret-token")
        );
        assert!(plan.extra_env.is_empty());
        let line = plan.redacted_shell_line();
        assert!(line.contains("--machine-token=***"));
        assert!(!line.contains("super-secret-token"));
        let cmd = build_command(&plan);
        // Command debug includes args; env extras are empty.
        let dbg = format!("{cmd:?}");
        assert!(dbg.contains("super-secret-token"));
        assert!(!dbg.contains("TERMINUS_MACHINE_TOKEN"));
    }

    #[test]
    fn login_chunks_redact_token() {
        let plan = crate::workflows::auth::plan_login(PathBuf::from("/bin/echo"), "sekrit-token");
        // echo will print the argv including the token; UI must redact.
        let hub = JobHub::new();
        let _job = spawn(hub.tx.clone(), plan.clone(), JobKind::User).expect("spawn");
        let events = drain_timeout(&hub.rx, Duration::from_secs(3));
        let mut log = String::new();
        for ev in events {
            if let JobEvent::Chunk { text, .. } = ev {
                log.push_str(&plan.redact_text(&text));
            }
        }
        assert!(!log.contains("sekrit-token"), "{log}");
    }

    #[test]
    fn omit_bytes_when_channel_full() {
        let (tx, rx) = mpsc::sync_channel(1);
        let id = JobId::new();
        let omitted = AtomicUsize::new(0);
        tx.send(JobEvent::Chunk {
            id,
            stream: Stream::Stdout,
            text: "fill".into(),
        })
        .unwrap();
        let err = try_send_chunk(&tx, id, "abcdef".into(), &omitted);
        assert!(err.is_err());
        assert_eq!(omitted.load(Ordering::SeqCst), 6);
        let _ = rx.recv();
        try_send_chunk(&tx, id, "ok".into(), &omitted).unwrap();
        let ev = rx.recv().unwrap();
        let JobEvent::Chunk { text, .. } = ev else {
            panic!("expected chunk");
        };
        assert!(
            text.contains("omitted 6 bytes") && text.contains("ok"),
            "{text}"
        );
    }

    #[test]
    fn dry_run_refuses_spawn() {
        let hub = JobHub::new();
        let mut plan = raw_plan("/bin/echo", &["x"]);
        plan.dry_run = true;
        let err = spawn(hub.tx, plan, JobKind::User).unwrap_err();
        assert!(err.to_string().contains("dry_run"));
    }
}
