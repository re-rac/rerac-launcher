//! Runs `rerac-extract` (contract §Extractor CLI) as a cancellable background job.
//!
//! stdout is read as JSON lines and forwarded to the page as `extractor://event`; the end of
//! the job is `extractor://finished`. Extraction writes into `games/<game>/data.staging/` and is
//! renamed over `data/` only after a clean exit, so a failed or cancelled run leaves the previous
//! install untouched. Every line is also written to `logs/<kind>-<game>-<unix time>.log`.
//!
//! `export` (game-side clarification 18) writes the optional Tier 2 exports (PNG, WAV, glTF, JSON)
//! from the installed data into a folder the user picked; it never touches `data/`.

use crate::contract::{parse_event_line, ErrorCode, ExtractInfo, ExtractorEvent, Stage, EXTRACT_INFO_FILE};
use serde::Serialize;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const EVENT: &str = "extractor://event";
pub const FINISHED: &str = "extractor://finished";
const PROGRESS_INTERVAL: Duration = Duration::from_millis(80);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum JobKind {
    Extract,
    Verify,
    Export,
}

impl JobKind {
    pub fn name(self) -> &'static str {
        match self {
            JobKind::Extract => "extract",
            JobKind::Verify => "verify",
            JobKind::Export => "export",
        }
    }
}

/// Export kinds the extractor accepts in `--what` (game-side clarification 18).
pub const EXPORT_KINDS: [&str; 6] = ["textures", "audio", "models", "levels", "collision", "text"];

/// What an export job writes and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSpec {
    pub to: PathBuf,
    /// A non-empty subset of [`EXPORT_KINDS`].
    pub what: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventPayload {
    pub job: u64,
    pub kind: JobKind,
    pub game: String,
    pub event: ExtractorEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FinishStatus {
    Ok,
    Error,
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
pub struct FinishedPayload {
    pub job: u64,
    pub kind: JobKind,
    pub game: String,
    pub status: FinishStatus,
    /// Contract error code (0 on success). Unknown exit codes are reported as they are; the
    /// page maps them with the same fallback as `ErrorCode::from_code`.
    pub code: i32,
    pub message: String,
    pub elapsed_ms: u64,
}

/// What a job needs; built by the command layer from the active version.
pub struct JobSpec {
    pub kind: JobKind,
    pub game: String,
    pub extractor: PathBuf,
    pub workdir: PathBuf,
    pub iso: Option<PathBuf>,
    pub ntsc_only: bool,
    pub data_dir: PathBuf,
    pub staging_dir: PathBuf,
    pub logs_dir: PathBuf,
    /// Set for `JobKind::Export`.
    pub export: Option<ExportSpec>,
}

/// Games whose extractor takes `--ntsc-only` (the "Skip PAL-only data" setting).
pub const NTSC_ONLY_GAMES: [&str; 1] = ["rac1"];

impl JobSpec {
    /// A job for `game` run by the extractor of version `v`, with the data root's folders.
    pub fn new(
        kind: JobKind,
        game: &str,
        v: &crate::versions::ResolvedVersion,
        layout: &crate::paths::Layout,
        iso: Option<PathBuf>,
        ntsc_only: bool,
    ) -> JobSpec {
        JobSpec {
            kind,
            game: game.to_string(),
            extractor: v.extractor.clone(),
            workdir: v.dir.clone(),
            iso,
            ntsc_only,
            data_dir: layout.game_data_dir(game),
            staging_dir: layout.game_staging_dir(game),
            logs_dir: layout.logs_dir(),
            export: None,
        }
    }

    pub fn args(&self) -> Vec<String> {
        let s = |p: &Path| p.to_string_lossy().into_owned();
        // `--ntsc-only` skips R&C1's PAL copies; Going Commando's extractor has no such option.
        let ntsc_only = self.ntsc_only && NTSC_ONLY_GAMES.contains(&self.game.as_str());
        match self.kind {
            JobKind::Extract => {
                let mut a = vec![
                    "extract".into(),
                    "--iso".into(),
                    s(self.iso.as_deref().unwrap_or(Path::new(""))),
                    "--out".into(),
                    s(&self.staging_dir),
                ];
                if ntsc_only {
                    a.push("--ntsc-only".into());
                }
                a.push("--json".into());
                a
            }
            JobKind::Verify => vec!["verify".into(), "--out".into(), s(&self.data_dir), "--json".into()],
            JobKind::Export => {
                let e = self.export.as_ref();
                vec![
                    "export".into(),
                    "--out".into(),
                    s(&self.data_dir),
                    "--to".into(),
                    e.map(|e| s(&e.to)).unwrap_or_default(),
                    "--what".into(),
                    e.map(|e| e.what.join(",")).unwrap_or_default(),
                    "--json".into(),
                ]
            }
        }
    }
}

struct Running {
    id: u64,
    kind: JobKind,
    game: String,
    child: Arc<Mutex<Child>>,
    cancelled: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct JobManager {
    next: AtomicU64,
    current: Arc<Mutex<Option<Running>>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobState {
    pub job: u64,
    pub kind: JobKind,
    pub game: String,
}

impl JobManager {
    pub fn current(&self) -> Option<JobState> {
        self.current
            .lock()
            .unwrap()
            .as_ref()
            .map(|r| JobState { job: r.id, kind: r.kind, game: r.game.clone() })
    }

    pub fn is_busy(&self) -> bool {
        self.current.lock().unwrap().is_some()
    }

    /// Starts a job. `emit_event` / `emit_finished` deliver to the page (Tauri events in the
    /// app, a collector in tests).
    pub fn start(
        &self,
        spec: JobSpec,
        emit_event: impl Fn(EventPayload) + Send + 'static,
        emit_finished: impl Fn(FinishedPayload) + Send + 'static,
    ) -> Result<u64, String> {
        let mut slot = self.current.lock().unwrap();
        if slot.is_some() {
            return Err("Another extractor job is already running.".into());
        }
        if spec.kind == JobKind::Export {
            let e = spec.export.as_ref().ok_or("Export job without a destination.")?;
            fs::create_dir_all(&e.to).map_err(|err| format!("Could not create {}: {err}", e.to.display()))?;
        }
        if spec.kind == JobKind::Extract {
            remove_dir_if_exists(&spec.staging_dir).map_err(|e| format!("Could not clear old staging folder: {e}"))?;
            fs::create_dir_all(&spec.staging_dir).map_err(|e| format!("Could not create {}: {e}", spec.staging_dir.display()))?;
        }
        let _ = fs::create_dir_all(&spec.logs_dir);
        let mut child = Command::new(&spec.extractor)
            .args(spec.args())
            .current_dir(&spec.workdir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Could not start the extractor ({}): {e}", spec.extractor.display()))?;

        let id = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        let child = Arc::new(Mutex::new(child));
        let cancelled = Arc::new(AtomicBool::new(false));
        *slot = Some(Running {
            id,
            kind: spec.kind,
            game: spec.game.clone(),
            child: child.clone(),
            cancelled: cancelled.clone(),
        });
        drop(slot);

        let current = self.current.clone();
        std::thread::spawn(move || {
            let started = Instant::now();
            let unix = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let kind_name = spec.kind.name();
            let mut log = fs::File::create(spec.logs_dir.join(format!("{kind_name}-{}-{unix}.log", spec.game))).ok();
            if let Some(l) = log.as_mut() {
                let _ = writeln!(l, "$ {} {}", spec.extractor.display(), spec.args().join(" "));
            }

            let stderr_tail = std::thread::spawn(move || {
                let mut s = String::new();
                let _ = BufReader::new(stderr).read_to_string(&mut s);
                s
            });

            let mut last_error: Option<(i32, String)> = None;
            // The real extractor finishes 4 GiB in seconds; forward at most ~12 progress lines a
            // second per stage, plus every stage change and every stage's final line.
            let mut last_progress: Option<(Instant, Stage)> = None;
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if let Some(l) = log.as_mut() {
                    let _ = writeln!(l, "{line}");
                }
                if let Some(event) = parse_event_line(&line) {
                    if let ExtractorEvent::Progress { stage, done, total, .. } = &event {
                        let now = Instant::now();
                        let recent = matches!(last_progress, Some((t, s)) if s == *stage
                            && now.duration_since(t) < PROGRESS_INTERVAL);
                        if recent && done < total {
                            continue;
                        }
                        last_progress = Some((now, *stage));
                    }
                    if let ExtractorEvent::Error { code, message } = &event {
                        last_error = Some((*code, message.clone()));
                    }
                    emit_event(EventPayload { job: id, kind: spec.kind, game: spec.game.clone(), event });
                }
            }

            let exit = wait_child(&child);
            let stderr_text = stderr_tail.join().unwrap_or_default();
            if let Some(l) = log.as_mut() {
                if !stderr_text.is_empty() {
                    let _ = writeln!(l, "--- stderr ---\n{stderr_text}");
                }
                let _ = writeln!(l, "--- exit: {exit:?} ---");
            }

            let (status, code, message) = if cancelled.load(Ordering::SeqCst) {
                (FinishStatus::Cancelled, 0, "Cancelled".to_string())
            } else {
                finish(&spec, exit, last_error, &stderr_text)
            };
            if spec.kind == JobKind::Extract && status != FinishStatus::Ok {
                let _ = remove_dir_if_exists(&spec.staging_dir);
            }
            *current.lock().unwrap() = None;
            emit_finished(FinishedPayload {
                job: id,
                kind: spec.kind,
                game: spec.game,
                status,
                code,
                message,
                elapsed_ms: started.elapsed().as_millis() as u64,
            });
        });
        Ok(id)
    }

    /// Kills the running job. Staging data is removed by the job thread.
    pub fn cancel(&self) -> bool {
        let slot = self.current.lock().unwrap();
        match slot.as_ref() {
            Some(r) => {
                r.cancelled.store(true, Ordering::SeqCst);
                let _ = r.child.lock().unwrap().kill();
                true
            }
            None => false,
        }
    }
}

fn wait_child(child: &Arc<Mutex<Child>>) -> Option<i32> {
    loop {
        {
            let mut c = child.lock().unwrap();
            match c.try_wait() {
                Ok(Some(status)) => return status.code(),
                Ok(None) => {}
                Err(_) => return None,
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Decides the outcome from the exit code (authoritative per the contract), the last `error`
/// line and stderr. On a clean extract, promotes staging to `data/`.
fn finish(
    spec: &JobSpec,
    exit: Option<i32>,
    last_error: Option<(i32, String)>,
    stderr: &str,
) -> (FinishStatus, i32, String) {
    let code = exit.unwrap_or(ErrorCode::Internal.code());
    if code != 0 {
        let message = last_error
            .filter(|(c, _)| *c == code)
            .map(|(_, m)| m)
            .or_else(|| stderr.lines().rev().find(|l| !l.trim().is_empty()).map(str::to_string))
            .unwrap_or_else(|| match exit {
                Some(c) => format!("The extractor exited with code {c}"),
                None => "The extractor was stopped by the system".to_string(),
            });
        return (FinishStatus::Error, code, message);
    }
    match spec.kind {
        JobKind::Verify => return (FinishStatus::Ok, 0, "All files match.".into()),
        JobKind::Export => {
            let to = spec.export.as_ref().map(|e| e.to.display().to_string()).unwrap_or_default();
            return (FinishStatus::Ok, 0, format!("Exported to {to}"));
        }
        JobKind::Extract => {}
    }
    match fs::read_to_string(spec.staging_dir.join(EXTRACT_INFO_FILE))
        .map_err(|_| format!("The extractor finished but wrote no {EXTRACT_INFO_FILE}"))
        .and_then(|t| ExtractInfo::parse(&t))
    {
        Ok(_) => match promote(&spec.staging_dir, &spec.data_dir) {
            Ok(()) => (FinishStatus::Ok, 0, "Extraction complete.".into()),
            Err(e) => (FinishStatus::Error, ErrorCode::WriteFailure.code(), e),
        },
        Err(e) => (FinishStatus::Error, ErrorCode::Internal.code(), e),
    }
}

/// Replaces `data` with `staging`: data → data.old, staging → data, delete data.old.
pub fn promote(staging: &Path, data: &Path) -> Result<(), String> {
    let old = data.with_extension("old");
    remove_dir_if_exists(&old).map_err(|e| e.to_string())?;
    if data.exists() {
        fs::rename(data, &old).map_err(|e| format!("Could not replace the old data: {e}"))?;
    }
    if let Err(e) = fs::rename(staging, data) {
        let _ = fs::rename(&old, data);
        return Err(format!("Could not move the new data into place: {e}"));
    }
    let _ = remove_dir_if_exists(&old);
    Ok(())
}

pub fn remove_dir_if_exists(p: &Path) -> std::io::Result<()> {
    match fs::remove_dir_all(p) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Reads `extract-info.json` of an installed game.
pub fn read_install(data_dir: &Path) -> Option<ExtractInfo> {
    let text = fs::read_to_string(data_dir.join(EXTRACT_INFO_FILE)).ok()?;
    ExtractInfo::parse(&text).ok()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::paths::tests::TempDir;
    use std::sync::mpsc;

    fn script(dir: &Path, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let p = dir.join("fake-extract");
        fs::write(&p, format!("#!/bin/sh\n{body}")).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    fn spec(t: &TempDir, extractor: PathBuf, kind: JobKind) -> JobSpec {
        JobSpec {
            kind,
            game: "rac1".into(),
            extractor,
            workdir: t.0.clone(),
            iso: Some(t.0.join("disc.iso")),
            ntsc_only: false,
            data_dir: t.0.join("games/rac1/data"),
            staging_dir: t.0.join("games/rac1/data.staging"),
            logs_dir: t.0.join("logs"),
            export: None,
        }
    }

    fn run(spec: JobSpec) -> (Vec<ExtractorEvent>, FinishedPayload) {
        let jm = JobManager::default();
        let (etx, erx) = mpsc::channel();
        let (ftx, frx) = mpsc::channel();
        jm.start(spec, move |e| etx.send(e.event).unwrap(), move |f| ftx.send(f).unwrap()).unwrap();
        let fin = frx.recv_timeout(Duration::from_secs(10)).unwrap();
        (erx.try_iter().collect(), fin)
    }

    #[test]
    fn args_follow_contract() {
        let t = TempDir::new("args");
        let mut s = spec(&t, "x".into(), JobKind::Extract);
        s.ntsc_only = true;
        let a = s.args();
        assert_eq!(a[0], "extract");
        assert_eq!(a[1], "--iso");
        assert_eq!(a[3], "--out");
        assert!(a[4].ends_with("data.staging"));
        assert_eq!(&a[5..], ["--ntsc-only", "--json"]);
        // Going Commando's extractor never gets it.
        let mut g = spec(&t, "x".into(), JobKind::Extract);
        g.game = "rac2".into();
        g.ntsc_only = true;
        assert_eq!(&g.args()[5..], ["--json"]);
        let v = spec(&t, "x".into(), JobKind::Verify).args();
        assert_eq!(v[0], "verify");
        assert!(v[2].ends_with("games/rac1/data"));
        assert_eq!(v[3], "--json");
        let mut e = spec(&t, "x".into(), JobKind::Export);
        e.export = Some(ExportSpec { to: t.0.join("out"), what: vec!["textures".into(), "audio".into()] });
        let a = e.args();
        assert_eq!((a[0].as_str(), a[1].as_str(), a[3].as_str(), a[5].as_str()), ("export", "--out", "--to", "--what"));
        assert!(a[2].ends_with("games/rac1/data") && a[4].ends_with("out"));
        assert_eq!((a[6].as_str(), a[7].as_str()), ("textures,audio", "--json"));
    }

    #[test]
    fn export_writes_to_its_folder_and_leaves_the_install_alone() {
        let t = TempDir::new("export");
        // $5 is the --to dir.
        let ex = script(
            &t.0,
            r#"echo '{"type":"progress","stage":"export","done":1,"total":2,"file":"levels/01/core_data.bin"}'
echo '{"format":1}' > "$5/export-info.json"
echo '{"type":"progress","stage":"export","done":2,"total":2,"file":"levels/01/core_data.bin"}'
echo '{"type":"done","elapsed_ms":5}'
"#,
        );
        let mut s = spec(&t, ex, JobKind::Export);
        let to = t.0.join("My Exports");
        s.export = Some(ExportSpec { to: to.clone(), what: vec!["textures".into()] });
        fs::create_dir_all(&s.data_dir).unwrap();
        fs::write(s.data_dir.join("keep.bin"), "keep").unwrap();
        let data = s.data_dir.clone();
        let (events, fin) = run(s);
        assert_eq!(fin.status, FinishStatus::Ok, "{}", fin.message);
        assert_eq!(fin.kind, JobKind::Export);
        assert!(fin.message.contains("My Exports"));
        assert!(matches!(events.last(), Some(ExtractorEvent::Done { .. })));
        assert!(events.iter().any(|e| matches!(e, ExtractorEvent::Progress { stage: Stage::Export, done: 2, .. })));
        assert!(to.join("export-info.json").is_file());
        assert!(data.join("keep.bin").exists());
        assert!(!t.0.join("games/rac1/data.staging").exists());
        assert!(fs::read_dir(t.0.join("logs")).unwrap().flatten().any(|e| e.file_name().to_string_lossy().starts_with("export-rac1-")));
    }

    #[test]
    fn successful_extract_promotes_staging() {
        let t = TempDir::new("ok");
        // $5 is the --out dir.
        let ex = script(
            &t.0,
            r#"echo '{"type":"progress","stage":"copy","done":1,"total":2,"file":"a"}'
echo 'not json'
echo '{"disc":"SCUS_971.99","data_format":1,"extractor_version":"t","ntsc_only":false,"files":1,"bytes":3}' > "$5/extract-info.json"
echo '{"type":"done","elapsed_ms":5}'
exit 0
"#,
        );
        let s = spec(&t, ex, JobKind::Extract);
        fs::create_dir_all(&s.data_dir).unwrap();
        fs::write(s.data_dir.join("old.bin"), "old").unwrap();
        let data = s.data_dir.clone();
        let (events, fin) = run(s);
        assert_eq!(fin.status, FinishStatus::Ok, "{}", fin.message);
        assert_eq!(events.len(), 3);
        assert!(matches!(events[1], ExtractorEvent::Info { .. }));
        assert!(read_install(&data).is_some());
        assert!(!data.join("old.bin").exists());
        assert!(!t.0.join("games/rac1/data.staging").exists());
        assert!(fs::read_dir(t.0.join("logs")).unwrap().count() == 1);
    }

    #[test]
    fn error_code_and_message_come_from_exit_and_error_line() {
        let t = TempDir::new("err");
        let ex = script(&t.0, "echo '{\"type\":\"error\",\"code\":21,\"message\":\"PAL disc\"}'\nexit 21\n");
        let s = spec(&t, ex, JobKind::Extract);
        fs::create_dir_all(&s.data_dir).unwrap();
        fs::write(s.data_dir.join("keep.bin"), "keep").unwrap();
        let data = s.data_dir.clone();
        let (_, fin) = run(s);
        assert_eq!(fin.status, FinishStatus::Error);
        assert_eq!(fin.code, 21);
        assert_eq!(fin.message, "PAL disc");
        assert!(data.join("keep.bin").exists(), "a failed run must not touch the old install");
        assert!(!t.0.join("games/rac1/data.staging").exists());
    }

    #[test]
    fn fast_progress_is_throttled_but_stage_ends_arrive() {
        let t = TempDir::new("throttle");
        let ex = script(
            &t.0,
            r#"i=0
while [ $i -lt 200 ]; do echo "{\"type\":\"progress\",\"stage\":\"verify\",\"done\":$i,\"total\":200,\"file\":\"f\"}"; i=$((i+1)); done
echo '{"type":"progress","stage":"verify","done":200,"total":200,"file":"f"}'
echo '{"type":"done","elapsed_ms":1}'
"#,
        );
        let (events, fin) = run(spec(&t, ex, JobKind::Verify));
        assert_eq!(fin.status, FinishStatus::Ok);
        let progress: Vec<_> = events.iter().filter(|e| matches!(e, ExtractorEvent::Progress { .. })).collect();
        assert!(progress.len() < 50, "{} progress events forwarded", progress.len());
        assert!(matches!(progress.last(), Some(ExtractorEvent::Progress { done: 200, total: 200, .. })));
    }

    #[test]
    fn exit_code_without_error_line_uses_stderr() {
        let t = TempDir::new("stderr");
        let ex = script(&t.0, "echo 'disk is full' >&2\nexit 31\n");
        let (_, fin) = run(spec(&t, ex, JobKind::Verify));
        assert_eq!((fin.status, fin.code, fin.message.as_str()), (FinishStatus::Error, 31, "disk is full"));
    }

    #[test]
    fn clean_exit_without_extract_info_is_an_error() {
        let t = TempDir::new("noinfo");
        let ex = script(&t.0, "exit 0\n");
        let (_, fin) = run(spec(&t, ex, JobKind::Extract));
        assert_eq!(fin.status, FinishStatus::Error);
        assert_eq!(fin.code, 99);
    }

    /// End to end against the mock tools in dev/mock (skipped when Node is not installed).
    #[test]
    fn mock_extractor_and_runtime_follow_the_contract() {
        if Command::new("node").arg("--version").output().is_err() {
            eprintln!("node not found; skipping");
            return;
        }
        let mock = Path::new(env!("CARGO_MANIFEST_DIR")).join("../dev/mock");
        let vref = crate::versions::VersionRef {
            source: crate::versions::SourceId::Development,
            id: mock.to_string_lossy().into(),
        };
        let info = crate::versions::validate(vref.clone(), mock.clone(), false);
        assert_eq!(info.problem, None);
        let v = crate::versions::resolve_dir(vref, mock.clone()).unwrap();

        let t = TempDir::new("mock");
        std::env::set_var("RERAC_MOCK_SECONDS", "0.2");
        let iso = t.0.join("Ratchet & Clank (USA).iso");
        fs::write(&iso, "").unwrap();
        let mut s = spec(&t, v.extractor.clone(), JobKind::Extract);
        s.workdir = mock.clone();
        s.iso = Some(iso);
        let data = s.data_dir.clone();
        let (events, fin) = run(s);
        assert_eq!(fin.status, FinishStatus::Ok, "{}", fin.message);
        assert!(events.iter().any(|e| matches!(e, ExtractorEvent::Disc { supported: true, .. })));
        assert!(matches!(events.last(), Some(ExtractorEvent::Done { .. })));
        assert_eq!(read_install(&data).unwrap().disc, "SCUS_971.99");

        let pal = t.0.join("Ratchet & Clank (Europe).iso");
        fs::write(&pal, "").unwrap();
        let mut s = spec(&t, v.extractor.clone(), JobKind::Extract);
        s.iso = Some(pal);
        let (_, fin) = run(s);
        assert_eq!((fin.status, fin.code), (FinishStatus::Error, 21));
        assert!(read_install(&data).is_some(), "failed run kept the install");

        let (_, fin) = run(spec(&t, v.extractor.clone(), JobKind::Verify));
        assert_eq!(fin.status, FinishStatus::Ok, "{}", fin.message);

        let out = crate::launch::command(&v.runtime, &mock, &data)
            .env("RERAC_MOCK_RUN_SECONDS", "0")
            .output()
            .unwrap();
        assert!(out.status.success());
        assert!(String::from_utf8_lossy(&out.stdout).contains("RC_DATA_DIR="));
    }

    /// Against the real game-side extractor and disc. Opt-in:
    /// `RERAC_REAL_EXTRACT=<rerac-extract> RERAC_REAL_ISO=<disc.iso> cargo test -- --ignored real_`
    /// Writes about 4 GiB into the system temp dir and deletes it afterwards.
    #[test]
    #[ignore]
    fn real_extractor_end_to_end() {
        let (Ok(bin), Ok(iso)) = (std::env::var("RERAC_REAL_EXTRACT"), std::env::var("RERAC_REAL_ISO")) else {
            eprintln!("RERAC_REAL_EXTRACT / RERAC_REAL_ISO not set; skipping");
            return;
        };
        let t = TempDir::new("real");
        let mut s = spec(&t, bin.clone().into(), JobKind::Extract);
        s.iso = Some(iso.into());
        let data = s.data_dir.clone();
        let jm = JobManager::default();
        let (etx, erx) = mpsc::channel();
        let (ftx, frx) = mpsc::channel();
        jm.start(s, move |e| etx.send(e.event).unwrap(), move |f| ftx.send(f).unwrap()).unwrap();
        let fin = frx.recv_timeout(Duration::from_secs(120)).unwrap();
        let events: Vec<_> = erx.try_iter().collect();
        assert_eq!(fin.status, FinishStatus::Ok, "{}", fin.message);
        assert!(events.iter().any(|e| matches!(e, ExtractorEvent::Disc { supported: true, .. })));
        let copies = events.iter().filter(|e| matches!(e, ExtractorEvent::Progress { stage: Stage::Copy, .. })).count();
        let info = read_install(&data).unwrap();
        eprintln!("extract: {} ms, {} copy events forwarded, {} files, {} bytes", fin.elapsed_ms, copies, info.files, info.bytes);
        assert_eq!(info.disc, "SCUS_971.99");

        let (_, fin) = run(spec(&t, bin.into(), JobKind::Verify));
        assert_eq!(fin.status, FinishStatus::Ok, "{}", fin.message);
        eprintln!("verify: {} ms", fin.elapsed_ms);
    }

    #[test]
    fn cancel_kills_and_cleans_up() {
        let t = TempDir::new("cancel");
        let ex = script(&t.0, "echo '{\"type\":\"info\",\"message\":\"started\"}'\nexec sleep 30\n");
        let s = spec(&t, ex, JobKind::Extract);
        let staging = s.staging_dir.clone();
        let jm = JobManager::default();
        let (ftx, frx) = mpsc::channel();
        let (etx, erx) = mpsc::channel();
        jm.start(s, move |e| etx.send(e).unwrap(), move |f| ftx.send(f).unwrap()).unwrap();
        erx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(jm.is_busy());
        assert!(jm.start(spec(&t, "x".into(), JobKind::Verify), |_| {}, |_| {}).is_err());
        assert!(jm.cancel());
        let fin = frx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(fin.status, FinishStatus::Cancelled);
        assert!(!staging.exists());
        assert!(!jm.is_busy());
    }
}
