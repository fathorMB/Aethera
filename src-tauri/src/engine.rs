//! Ciclo di vita di un processo `llama-server`: avvio in job object, stato da `/health` e `/props`,
//! memoria misurata dopo il caricamento, telemetria per richiesta, stato «in uso», arresto.
//! Un motore alla volta; nessun riavvio implicito.

use crate::cmdline;
use crate::conditions::Conditions;
use crate::job::Job;
use crate::machine::ResolvedBuild;
use crate::manifest::{
    self, CommandSection, EngineSection, ExitSection, MachineSection, Manifest, ModelSection, RunSection, ServerSection,
};
use crate::memory::{self, MemoryAfter, MemorySection};
use crate::profile::{Override, Profile};
use crate::provenance;
use crate::system::{self, SystemReport};
use crate::telemetry::{self, Counters, LogParser, LogTail, Record, Reference, Summary};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Richieste negli ultimi secondi rendono il motore «in uso» anche con lo slot libero.
pub const IN_USE_WINDOW_S: u64 = 30;
/// Attesa dopo «pronto» prima di leggere la memoria: i buffer di calcolo si allocano subito dopo.
const MEMORY_SETTLE: Duration = Duration::from_secs(3);
pub const LOCK_TTL_DEFAULT_S: u64 = 600;
pub const LOCK_TTL_MAX_S: u64 = 24 * 3600;

pub struct StartRequest {
    pub run_id: String,
    pub run_dir: PathBuf,
    /// Nome del profilo su disco da cui si parte (vuoto se non c'è): serve a «riavvia con la stessa riga».
    pub base: String,
    pub profile: Profile,
    pub profile_file: Option<PathBuf>,
    pub overrides: Vec<Override>,
    pub invalidates_cache: bool,
    pub build: ResolvedBuild,
    pub model_path: PathBuf,
    pub model_size: u64,
    pub args: Vec<String>,
    pub machine_name: String,
    pub ram_margin_gib: f64,
    pub system: SystemReport,
    pub conditions: Conditions,
    /// Mediana del decode degli avvii precedenti con le stesse condizioni: la soglia «degradato».
    pub reference: Option<Reference>,
    /// Che cosa è cambiato dall'avvio precedente sulla stessa macchina: `driver GPU a → b`.
    pub conditions_changed: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunInfo {
    pub run_id: String,
    pub profile: String,
    pub base: String,
    pub base_url: String,
    pub started_at: String,
    pub pid: u32,
    pub build: String,
    pub command_line: String,
    pub log_path: String,
    pub manifest_path: String,
    pub ctx_declared: u32,
    pub n_parallel: u32,
    pub overrides: Vec<Override>,
    pub invalidates_cache: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finished {
    pub run: RunInfo,
    pub code: Option<i32>,
    pub by_user: bool,
    pub left_running: bool,
    pub ended_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LockView {
    pub id: String,
    pub client: String,
    pub label: Option<String>,
    pub since: String,
    pub expires_in_s: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LockRequest {
    pub client: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub ttl_s: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Usage {
    pub in_use: bool,
    /// Perché è in uso, in parole: slot attivo, richiesta recente, lock, protezione.
    pub reasons: Vec<String>,
    pub protected: bool,
    pub slot_processing: Option<bool>,
    pub last_request_s: Option<u64>,
    pub locks: Vec<LockView>,
}

/// Un `llama-server` in ascolto su una porta dei profili, non avviato da questo Aethera.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Orphan {
    pub pid: u32,
    pub port: u16,
    pub base_url: String,
    pub process: String,
    pub alias: Option<String>,
    pub ctx: Option<u32>,
    pub slot_processing: Option<bool>,
    /// Lasciato acceso all'uscita da un Aethera precedente sulla stessa porta.
    pub left_by_aethera: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum EngineStatus {
    Off {
        last: Option<Finished>,
    },
    Loading {
        run: RunInfo,
        elapsed_s: u64,
        health: Option<u16>,
        usage: Usage,
    },
    Ready {
        run: RunInfo,
        uptime_s: u64,
        load_ms: u64,
        ctx_served: Option<u32>,
        alias_served: Option<String>,
        divergences: Vec<String>,
        usage: Usage,
        telemetry: Summary,
        counters: Option<Counters>,
        memory: Option<MemorySection>,
        degraded: Vec<String>,
        conditions: Option<Conditions>,
        conditions_changed: Vec<String>,
        reference: Option<Reference>,
    },
    Exited {
        finished: Finished,
    },
    Orphan {
        orphan: Orphan,
        last: Option<Finished>,
    },
}

enum Phase {
    Loading { health: Option<u16> },
    Ready { load_ms: u64, ctx_served: Option<u32>, alias_served: Option<String>, at: Instant },
}

struct Lock {
    id: String,
    client: String,
    label: Option<String>,
    since: String,
    expires: Instant,
}

struct Running {
    child: Child,
    job: Job,
    info: RunInfo,
    run_dir: PathBuf,
    started: Instant,
    phase: Phase,
    manifest: Manifest,
    manifest_path: PathBuf,
    records: Vec<Record>,
    counters: Option<Counters>,
    /// Token dalla cache per task, letti da `/slots` mentre la richiesta era in corso.
    slot_cache: BTreeMap<i64, u32>,
    slot_processing: Option<bool>,
    last_activity: Option<Instant>,
    locks: Vec<Lock>,
    reference: Option<Reference>,
    conditions_changed: Vec<String>,
}

#[derive(Default)]
struct Inner {
    current: Option<Running>,
    last: Option<Finished>,
    /// Profilo base ed effettivo dell'ultimo avvio: «riavvia con la stessa riga».
    source: Option<(String, Profile)>,
    protected: bool,
    orphan: Option<Orphan>,
}

#[derive(Clone, Default)]
pub struct Engine {
    inner: Arc<Mutex<Inner>>,
}

pub struct BinaryVersion {
    pub text: String,
    pub build: Option<String>,
    pub commit: Option<String>,
}

/// `version: 0.4.0-dev (build 10809, commit 5266f24da)` → `b10809`, `5266f24da`.
pub fn parse_version(text: &str) -> BinaryVersion {
    let after = |key: &str, keep: fn(char) -> bool| {
        text.find(key)
            .map(|i| text[i + key.len()..].chars().take_while(|c| keep(*c)).collect::<String>())
            .filter(|s| !s.is_empty())
    };
    BinaryVersion {
        text: text.replace('\r', "").trim().to_string(),
        build: after("(build ", |c| c.is_ascii_digit()).map(|d| format!("b{d}")),
        commit: after("commit ", |c| c.is_ascii_alphanumeric()),
    }
}

/// Il binario è la build che il profilo chiede? Per una build di ggml-org basta `--version`; per
/// una del fork (`b10991+moro1`) `--version` dice il tag e la serie deve venire dalla provenienza.
/// Una build del fork non passa mai per una di ggml-org, e viceversa.
pub fn check_build(
    binary: &Path,
    declared: &str,
    version: &BinaryVersion,
    provenance: Option<&Result<provenance::Provenance, String>>,
) -> Result<(), String> {
    let Some(found) = &version.build else {
        return Err(format!("build non leggibile da --version: {}", version.text));
    };
    let base = crate::machine::parse_build_tag(declared).map(|t| t.base).unwrap_or_else(|| declared.to_string());
    if *found != base {
        return Err(format!("{} è la build {found}, il profilo dichiara {declared}", binary.display()));
    }
    let wants_series = base != declared;
    match (wants_series, provenance) {
        (false, None) => Ok(()),
        (false, Some(Ok(p))) => Err(format!(
            "{} è la build del fork {} (serie {}): un profilo che dichiara {declared} usa solo la build di ggml-org, quella patchata va chiesta per nome",
            binary.display(),
            p.label(),
            p.series()
        )),
        (false, Some(Err(e))) | (true, Some(Err(e))) => {
            Err(format!("la build ha un file di provenienza che non si legge, quindi non si sa che serie sia: {e}"))
        }
        (true, None) => Err(format!(
            "il profilo dichiara {declared} ma accanto a {} non c'è {}: non è una build del fork",
            binary.display(),
            provenance::PROVENANCE_FILE
        )),
        (true, Some(Ok(p))) if p.label() == declared => Ok(()),
        (true, Some(Ok(p))) => Err(format!(
            "{} è la build del fork {}, il profilo dichiara {declared}",
            binary.display(),
            p.label()
        )),
    }
}

pub fn read_version(binary: &Path) -> Result<BinaryVersion, String> {
    let mut cmd = Command::new(binary);
    cmd.arg("--version").stdin(Stdio::null());
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let out = cmd.output().map_err(|e| format!("{} --version: {e}", binary.display()))?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    Ok(parse_version(&text))
}

fn port_in_use(host: &str, port: u16) -> bool {
    (host, port)
        .to_socket_addrs()
        .map(|addrs| addrs.into_iter().any(|a| TcpStream::connect_timeout(&a, Duration::from_millis(300)).is_ok()))
        .unwrap_or(false)
}

fn http_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(2)))
        .http_status_as_error(false)
        .build()
        .into()
}

fn http_status(agent: &ureq::Agent, url: &str) -> Option<u16> {
    agent.get(url).call().ok().map(|r| r.status().as_u16())
}

fn http_text(agent: &ureq::Agent, url: &str) -> Option<String> {
    let mut r = agent.get(url).call().ok()?;
    if r.status().as_u16() != 200 {
        return None;
    }
    r.body_mut().read_to_string().ok()
}

fn http_json(agent: &ureq::Agent, url: &str) -> Option<Value> {
    serde_json::from_str(&http_text(agent, url)?).ok()
}

fn slots_processing(v: &Value) -> Option<bool> {
    let slots = v.as_array()?;
    Some(slots.iter().any(|s| s["is_processing"].as_bool() == Some(true)))
}

fn ctx_from_props(v: &Value) -> Option<u32> {
    v["default_generation_settings"]["n_ctx"].as_u64().or_else(|| v["n_ctx"].as_u64()).map(|n| n as u32)
}

fn divergences(info: &RunInfo, ctx_served: Option<u32>, alias_served: Option<&str>, memory: Option<&MemorySection>) -> Vec<String> {
    let mut d = Vec::new();
    match ctx_served {
        Some(c) if c != info.ctx_declared => d.push(format!("contesto servito {c} ≠ dichiarato {}", info.ctx_declared)),
        None => d.push("contesto servito sconosciuto: /props non lo espone".into()),
        _ => {}
    }
    if let Some(a) = alias_served {
        if a != info.profile {
            d.push(format!("alias servito «{a}» ≠ profilo «{}»", info.profile));
        }
    }
    if let Some(after) = memory.and_then(|m| m.after_load.as_ref()) {
        if after.double_copy == Some(true) {
            d.push(format!(
                "doppia copia dei pesi: working set {:.1} GiB dopo il caricamento",
                after.working_set_gib.unwrap_or_default()
            ));
        }
    }
    d
}

fn now() -> String {
    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
}

fn next_lock_id() -> String {
    static N: AtomicU64 = AtomicU64::new(1);
    format!("l-{}-{}", chrono::Local::now().format("%H%M%S"), N.fetch_add(1, Ordering::Relaxed))
}

fn finish(mut r: Running, code: Option<i32>, by_user: bool, left_running: bool) -> Finished {
    let ended_at = now();
    r.manifest.exit = Some(ExitSection { at: ended_at.clone(), code, by_user, left_running });
    let _ = manifest::write(&r.manifest_path, &r.manifest);
    if left_running {
        r.job.release();
    }
    Finished { run: r.info.clone(), code, by_user, left_running, ended_at }
}

fn usage_of(protected: bool, r: Option<&mut Running>) -> Usage {
    let mut u = Usage { protected, ..Default::default() };
    if protected {
        u.reasons.push("protetto a mano".into());
    }
    if let Some(r) = r {
        let t = Instant::now();
        r.locks.retain(|l| l.expires > t);
        u.slot_processing = r.slot_processing;
        if r.slot_processing == Some(true) {
            u.reasons.push("slot attivo".into());
        }
        u.last_request_s = r.last_activity.map(|a| a.elapsed().as_secs());
        if let Some(s) = u.last_request_s.filter(|s| *s < IN_USE_WINDOW_S) {
            if r.slot_processing != Some(true) {
                u.reasons.push(format!("richiesta {s} s fa"));
            }
        }
        for l in &r.locks {
            u.reasons.push(match &l.label {
                Some(label) => format!("lock {} · {label}", l.client),
                None => format!("lock {}", l.client),
            });
            u.locks.push(LockView {
                id: l.id.clone(),
                client: l.client.clone(),
                label: l.label.clone(),
                since: l.since.clone(),
                expires_in_s: l.expires.saturating_duration_since(t).as_secs(),
            });
        }
    }
    u.in_use = !u.reasons.is_empty();
    u
}

impl Engine {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn is_running(&self) -> bool {
        self.lock().current.is_some()
    }

    pub fn status(&self) -> EngineStatus {
        let mut g = self.lock();
        let protected = g.protected;
        match g.current.as_mut() {
            Some(r) => {
                let usage = usage_of(protected, Some(r));
                match &r.phase {
                    Phase::Loading { health } => EngineStatus::Loading {
                        run: r.info.clone(),
                        elapsed_s: r.started.elapsed().as_secs(),
                        health: *health,
                        usage,
                    },
                    Phase::Ready { load_ms, ctx_served, alias_served, .. } => {
                        let uptime_s = r.started.elapsed().as_secs();
                        let ubatch = Some(r.manifest.effective_profile.server.ubatch);
                        EngineStatus::Ready {
                            run: r.info.clone(),
                            uptime_s,
                            load_ms: *load_ms,
                            ctx_served: *ctx_served,
                            alias_served: alias_served.clone(),
                            divergences: divergences(&r.info, *ctx_served, alias_served.as_deref(), r.manifest.memory.as_ref()),
                            usage,
                            telemetry: telemetry::summarize(&r.records, telemetry::RECENT, ubatch),
                            counters: r.counters,
                            memory: r.manifest.memory.clone(),
                            degraded: telemetry::degraded(uptime_s, &r.records, r.reference.as_ref()),
                            conditions: r.manifest.conditions.clone(),
                            conditions_changed: r.conditions_changed.clone(),
                            reference: r.reference.clone(),
                        }
                    }
                }
            }
            None => match (&g.orphan, &g.last) {
                (Some(o), last) => EngineStatus::Orphan { orphan: o.clone(), last: last.clone() },
                (None, Some(f)) if !f.by_user => EngineStatus::Exited { finished: f.clone() },
                (None, other) => EngineStatus::Off { last: other.clone() },
            },
        }
    }

    pub fn usage(&self) -> Usage {
        let mut g = self.lock();
        let protected = g.protected;
        usage_of(protected, g.current.as_mut())
    }

    pub fn set_protected(&self, on: bool) {
        self.lock().protected = on;
    }

    pub fn log_path(&self) -> Option<PathBuf> {
        let g = self.lock();
        g.current
            .as_ref()
            .map(|r| r.info.log_path.clone())
            .or_else(|| g.last.as_ref().map(|f| f.run.log_path.clone()))
            .map(PathBuf::from)
    }

    /// Profilo base ed effettivo dell'avvio corrente o dell'ultimo.
    pub fn source(&self) -> Option<(String, Profile)> {
        self.lock().source.clone()
    }

    pub fn manifest(&self) -> Option<Manifest> {
        self.lock().current.as_ref().map(|r| r.manifest.clone())
    }

    /// Ultime `n` richieste dell'avvio corrente, con il riepilogo sulla stessa finestra.
    pub fn recent(&self, n: usize) -> Option<(String, Summary, Vec<Record>)> {
        let g = self.lock();
        let r = g.current.as_ref()?;
        let from = r.records.len().saturating_sub(n);
        let ubatch = Some(r.manifest.effective_profile.server.ubatch);
        Some((r.info.run_id.clone(), telemetry::summarize(&r.records, n, ubatch), r.records[from..].to_vec()))
    }

    pub fn start(&self, req: StartRequest) -> Result<RunInfo, String> {
        if self.is_running() {
            return Err("un motore è già acceso: nella v1 se ne avvia uno alla volta".into());
        }
        let s = &req.profile.server;
        if port_in_use(&s.host, s.port) {
            return Err(format!(
                "{}:{} risponde già: la porta è occupata da un altro processo (forse un llama-server non avviato da Aethera)",
                s.host, s.port
            ));
        }
        let version = read_version(&req.build.binary)?;
        // M-14: una build del fork dice solo il tag in --version; la serie sta nella provenienza.
        let provenance = provenance::read(&req.build.dir);
        check_build(&req.build.binary, &req.profile.runtime.build, &version, provenance.as_ref())?;
        // VRAM libera secondo il backend, prima che il modello la occupi.
        let devices = memory::list_devices(&req.build.binary).unwrap_or_default();
        let memory_before = memory::before(&devices, system::probe().ram_available_gib().or(req.system.ram_available_gib));

        fs::create_dir_all(&req.run_dir).map_err(|e| format!("{}: {e}", req.run_dir.display()))?;
        if s.slot_save {
            let slots = req.run_dir.join("slots");
            fs::create_dir_all(&slots).map_err(|e| format!("{}: {e}", slots.display()))?;
        }
        let log_path = req.run_dir.join("server.log");
        let log = File::create(&log_path).map_err(|e| format!("{}: {e}", log_path.display()))?;
        let log_err = log.try_clone().map_err(|e| e.to_string())?;

        let mut cmd = Command::new(&req.build.binary);
        cmd.args(&req.args)
            .current_dir(&req.build.dir)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(log_err));
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

        let job = Job::kill_on_close()?;
        let started_at = now();
        let started = Instant::now();
        let mut child = cmd.spawn().map_err(|e| format!("avvio di {} non riuscito: {e}", req.build.binary.display()))?;
        if let Err(e) = job.assign(&child) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(e);
        }

        let command_line = cmdline::render_line(&req.build.binary, &req.args);
        let manifest_path = req.run_dir.join("manifest.toml");
        // M-14: la serie di patch della build entra nel manifest e fra le condizioni. Chi chiama
        // può averla già messa (la finestra la usa per il riferimento); se manca la si legge qui.
        let mut conditions = req.conditions.clone();
        if conditions.build_series.is_none() {
            conditions.build_series = Some(provenance::series_of(provenance.as_ref()));
        }
        let (provenance, provenance_error) = match provenance {
            Some(Ok(p)) => (Some(p), None),
            Some(Err(e)) => (None, Some(e)),
            None => (None, None),
        };
        let info = RunInfo {
            run_id: req.run_id.clone(),
            profile: req.profile.name.clone(),
            base: req.base.clone(),
            base_url: format!("http://{}:{}", s.host, s.port),
            started_at: started_at.clone(),
            pid: child.id(),
            build: req.profile.runtime.build.clone(),
            command_line: command_line.clone(),
            log_path: log_path.display().to_string(),
            manifest_path: manifest_path.display().to_string(),
            ctx_declared: s.ctx,
            n_parallel: s.n_parallel,
            overrides: req.overrides.clone(),
            invalidates_cache: req.invalidates_cache,
        };
        let manifest = Manifest {
            schema_version: manifest::MANIFEST_SCHEMA,
            run: RunSection {
                id: req.run_id.clone(),
                started: started_at,
                machine: req.machine_name.clone(),
                profile: req.profile.name.clone(),
                profile_file: req.profile_file.as_ref().map(|p| p.display().to_string()),
                invalidates_cache: req.invalidates_cache,
                log: log_path.display().to_string(),
            },
            engine: EngineSection {
                build_declared: req.profile.runtime.build.clone(),
                build: version.build.clone(),
                commit: version.commit.clone(),
                backend: req.profile.runtime.backend.clone(),
                build_id: req.build.id.clone(),
                binary: req.build.binary.display().to_string(),
                version_text: Some(version.text.clone()),
                provenance,
                provenance_error,
            },
            model: ModelSection {
                file: req.profile.model.file.clone(),
                path: req.model_path.display().to_string(),
                size_bytes: req.model_size,
                size_gb: (req.model_size as f64 / 1e7).round() / 100.0,
                sha256_declared: req.profile.model.sha256.clone(),
                sha256_verified: None,
            },
            server: ServerSection {
                host: s.host.clone(),
                port: s.port,
                alias: req.profile.name.clone(),
                ctx_declared: s.ctx,
                ctx_served: None,
                alias_served: None,
                ready_at: None,
                load_ms: None,
            },
            command: CommandSection {
                line: command_line,
                argv: std::iter::once(req.build.binary.display().to_string()).chain(req.args.iter().cloned()).collect(),
            },
            machine: MachineSection {
                hostname: req.system.hostname.clone(),
                os: req.system.os.clone(),
                cpu: req.system.cpu.clone(),
                gpus: req
                    .system
                    .gpus
                    .iter()
                    .map(|g| match g.dedicated_gib {
                        Some(d) => format!("{} · {d} GiB dedicati", g.name),
                        None => g.name.clone(),
                    })
                    .collect(),
                ram_total_gib: req.system.ram_total_gib,
                ram_available_gib_before: memory_before.ram_available_gib,
            },
            memory: Some(MemorySection { before: Some(memory_before), after_load: None }),
            conditions: Some(conditions),
            overrides: req.overrides.clone(),
            exit: None,
            effective_profile: req.profile.clone(),
        };
        if let Err(e) = manifest::write(&manifest_path, &manifest) {
            job.terminate();
            let _ = child.wait();
            return Err(e);
        }

        {
            let mut g = self.lock();
            g.orphan = None;
            g.source = Some((req.base.clone(), req.profile.clone()));
            g.current = Some(Running {
                child,
                job,
                info: info.clone(),
                run_dir: req.run_dir.clone(),
                started,
                phase: Phase::Loading { health: None },
                manifest,
                manifest_path,
                records: Vec::new(),
                counters: None,
                slot_cache: BTreeMap::new(),
                slot_processing: None,
                last_activity: None,
                locks: Vec::new(),
                reference: req.reference.clone(),
                conditions_changed: req.conditions_changed.clone(),
            });
        }
        let engine = self.clone();
        let run_id = info.run_id.clone();
        let margin = req.ram_margin_gib;
        std::thread::spawn(move || engine.monitor(run_id, log_path, margin));
        Ok(info)
    }

    /// Controlla l'uscita del processo; finché carica, `/health` e poi `/props` una volta. Da pronto:
    /// memoria dopo il caricamento, righe nuove del log per la telemetria, `/slots` e `/metrics`.
    fn monitor(&self, run_id: String, log_path: PathBuf, ram_margin_gib: f64) {
        let agent = http_agent();
        let probe = system::probe();
        let mut tail = LogTail::new(log_path);
        let mut parser = LogParser::default();
        let mut tick: u64 = 0;
        loop {
            std::thread::sleep(Duration::from_millis(250));
            tick += 1;
            let (base_url, loading, pid, n_parallel, memory_due, slot_busy) = {
                let mut g = self.lock();
                let exit = match g.current.as_mut() {
                    Some(r) if r.info.run_id == run_id => r.child.try_wait().ok().flatten().map(|s| s.code()),
                    _ => return,
                };
                if let Some(code) = exit {
                    if let Some(r) = g.current.take() {
                        g.last = Some(finish(r, code, false, false));
                    }
                    return;
                }
                let r = g.current.as_ref().expect("motore corrente");
                let memory_due = match &r.phase {
                    Phase::Ready { at, .. } => {
                        at.elapsed() >= MEMORY_SETTLE && r.manifest.memory.as_ref().map_or(true, |m| m.after_load.is_none())
                    }
                    Phase::Loading { .. } => false,
                };
                (
                    r.info.base_url.clone(),
                    matches!(r.phase, Phase::Loading { .. }),
                    r.info.pid,
                    r.info.n_parallel,
                    memory_due,
                    r.slot_processing,
                )
            };

            // Una richiesta partita dopo l'ultimo rilascio ha già mosso il contatore della cache.
            let mut timings = Vec::new();
            let mut launched_after_release = false;
            for line in tail.read_lines() {
                if let Some(t) = parser.feed(&line) {
                    timings.push(t);
                    launched_after_release = false;
                } else if telemetry::is_launch(&line) && !timings.is_empty() {
                    launched_after_release = true;
                }
            }

            if loading {
                if tick % 2 != 0 {
                    continue;
                }
                let health = http_status(&agent, &format!("{base_url}/health"));
                let props = if health == Some(200) { http_json(&agent, &format!("{base_url}/props")) } else { None };
                let mut g = self.lock();
                let Some(r) = g.current.as_mut().filter(|r| r.info.run_id == run_id) else { return };
                if health != Some(200) {
                    r.phase = Phase::Loading { health };
                    continue;
                }
                let ctx_served = props.as_ref().and_then(ctx_from_props);
                let alias_served = props.as_ref().and_then(|v| v["model_alias"].as_str().map(str::to_string));
                let load_ms = r.started.elapsed().as_millis() as u64;
                r.manifest.server.ctx_served = ctx_served;
                r.manifest.server.alias_served = alias_served.clone();
                r.manifest.server.load_ms = Some(load_ms);
                r.manifest.server.ready_at = Some(now());
                let _ = manifest::write(&r.manifest_path, &r.manifest);
                r.phase = Phase::Ready { load_ms, ctx_served, alias_served, at: Instant::now() };
                continue;
            }

            // Con lo slot al lavoro i contatori possono contenere mezza richiesta: si rileggono a richiesta chiusa.
            let want_metrics = !timings.is_empty() || (tick % 20 == 0 && slot_busy != Some(true));
            let counters = if want_metrics {
                http_text(&agent, &format!("{base_url}/metrics")).and_then(|t| Counters::from_metrics(&telemetry::parse_metrics(&t)))
            } else {
                None
            };
            // Righe scritte mentre si leggevano i contatori: richieste chiuse dopo la lettura non usano
            // quella differenza, e un avvio nuovo la rende non attribuibile.
            let mut late = Vec::new();
            if !timings.is_empty() {
                for line in tail.read_lines() {
                    if let Some(t) = parser.feed(&line) {
                        late.push(t);
                    } else if telemetry::is_launch(&line) {
                        launched_after_release = true;
                    }
                }
            }
            let slots = http_json(&agent, &format!("{base_url}/slots"));
            let measured = memory_due.then(|| {
                let pm = probe.process_memory(pid);
                (pm, probe.ram_available_gib())
            });

            let mut g = self.lock();
            let Some(r) = g.current.as_mut().filter(|r| r.info.run_id == run_id) else { return };
            if let Some(v) = slots.as_ref() {
                for (task, cache) in telemetry::slot_caches(v) {
                    r.slot_cache.insert(task, cache);
                }
                // Richieste interrotte senza tempi non lasciano la mappa crescere.
                while r.slot_cache.len() > 64 {
                    r.slot_cache.pop_first();
                }
            }
            let finished = timings.len() + late.len();
            if finished > 0 {
                let by_metrics = if n_parallel == 1 && !launched_after_release {
                    telemetry::attribute_cache(&timings, r.counters, counters)
                } else {
                    vec![None; timings.len()]
                };
                let late_none = vec![None; late.len()];
                // Il log per primo; /slots e /metrics restano per le build che non scrivono n_tokens.
                let client = {
                    let t = Instant::now();
                    let mut clients: Vec<&str> = r.locks.iter().filter(|l| l.expires > t).map(|l| l.client.as_str()).collect();
                    clients.dedup();
                    (clients.len() == 1).then(|| clients[0].to_string())
                };
                for (t, from_metrics) in timings.into_iter().zip(by_metrics).chain(late.into_iter().zip(late_none)) {
                    let from_slots = r.slot_cache.remove(&t.task);
                    let (cache_n, from) = match (t.cache_from_log(), from_slots, from_metrics) {
                        (Some(c), _, _) => (Some(c), Some("log")),
                        (None, Some(c), _) => (Some(c), Some("slots")),
                        (None, None, Some(c)) => (Some(c), Some("metrics")),
                        _ => (None, None),
                    };
                    let rec = t.into_record(now(), cache_n, from, client.clone());
                    let _ = telemetry::append(&r.run_dir, &rec);
                    r.records.push(rec);
                }
                r.last_activity = Some(Instant::now());
            }
            if counters.is_some() {
                r.counters = counters;
            }
            r.slot_processing = slots.as_ref().and_then(slots_processing);
            if r.slot_processing == Some(true) {
                r.last_activity = Some(Instant::now());
            }
            if let Some((pm, ram)) = measured {
                let after = MemoryAfter {
                    at: now(),
                    after_ms: r.started.elapsed().as_millis() as u64,
                    vram_dedicated_gib: pm.gpu_dedicated_gib,
                    vram_shared_gib: pm.gpu_shared_gib,
                    working_set_gib: pm.working_set_gib,
                    ram_available_gib: ram,
                    double_copy: memory::double_copy(pm.working_set_gib, r.manifest.model.size_bytes),
                    ram_margin_gib,
                    margin_ok: ram.map(|x| x >= ram_margin_gib),
                };
                r.manifest.memory.get_or_insert_with(MemorySection::default).after_load = Some(after);
                let _ = manifest::write(&r.manifest_path, &r.manifest);
            }
        }
    }

    /// Ferma il motore se non è in uso: slot attivo, richieste recenti, lock o protezione lo impediscono.
    pub fn stop(&self) -> Result<(), String> {
        let mut g = self.lock();
        let protected = g.protected;
        let usage = usage_of(protected, g.current.as_mut());
        let Some(r) = g.current.as_mut() else { return Err("nessun motore acceso".into()) };
        if usage.in_use {
            return Err(format!("motore in uso ({}): arresto rifiutato", usage.reasons.join(", ")));
        }
        r.job.terminate();
        let _ = r.child.kill();
        let code = r.child.wait().ok().and_then(|s| s.code());
        let r = g.current.take().expect("motore corrente");
        g.last = Some(finish(r, code, true, false));
        Ok(())
    }

    /// All'uscita con «lascia acceso»: il processo sopravvive ad Aethera.
    pub fn detach(&self) {
        let mut g = self.lock();
        if let Some(r) = g.current.take() {
            g.last = Some(finish(r, None, true, true));
        }
    }

    pub fn acquire_lock(&self, req: LockRequest) -> Result<(String, LockView), String> {
        let client = req.client.trim().to_string();
        if client.is_empty() || client.len() > 80 {
            return Err("client obbligatorio (al massimo 80 caratteri)".into());
        }
        let label = req.label.map(|l| l.trim().chars().take(120).collect::<String>()).filter(|l| !l.is_empty());
        let ttl = req.ttl_s.unwrap_or(LOCK_TTL_DEFAULT_S).clamp(1, LOCK_TTL_MAX_S);
        let mut g = self.lock();
        let Some(r) = g.current.as_mut() else { return Err("nessun motore acceso".into()) };
        let expires = Instant::now() + Duration::from_secs(ttl);
        // Lo stesso client con la stessa etichetta rinnova il suo lock invece di accumularne.
        let id = match r.locks.iter_mut().find(|l| l.client == client && l.label == label && l.expires > Instant::now()) {
            Some(l) => {
                l.expires = expires;
                l.id.clone()
            }
            None => {
                let id = next_lock_id();
                r.locks.push(Lock { id: id.clone(), client, label, since: now(), expires });
                id
            }
        };
        let run_id = r.info.run_id.clone();
        let protected = g.protected;
        let usage = usage_of(protected, g.current.as_mut());
        let view = usage.locks.into_iter().find(|l| l.id == id).expect("lock appena scritto");
        Ok((run_id, view))
    }

    /// Rilascia per id o tutti i lock di un client; restituisce quanti ne ha tolti.
    pub fn release_lock(&self, id: Option<&str>, client: Option<&str>) -> usize {
        let mut g = self.lock();
        let Some(r) = g.current.as_mut() else { return 0 };
        let before = r.locks.len();
        r.locks.retain(|l| !(id == Some(l.id.as_str()) || (id.is_none() && client == Some(l.client.as_str()))));
        before - r.locks.len()
    }

    /// Cerca un `llama-server` non gestito sulle porte date; solo con il motore spento.
    pub fn scan_orphans(&self, endpoints: &[(String, u16)]) {
        if self.is_running() {
            return;
        }
        let probe = system::probe();
        let agent = http_agent();
        let mut found = None;
        for (host, port) in endpoints {
            let Some(pid) = probe.listening_pid(*port) else { continue };
            let Some(process) = probe.process_name(pid) else { continue };
            if !process.to_ascii_lowercase().starts_with("llama-server") {
                continue;
            }
            let base_url = format!("http://{host}:{port}");
            let props = http_json(&agent, &format!("{base_url}/props"));
            let slots = http_json(&agent, &format!("{base_url}/slots"));
            found = Some(Orphan {
                pid,
                port: *port,
                base_url,
                process,
                alias: props.as_ref().and_then(|v| v["model_alias"].as_str().map(str::to_string)),
                ctx: props.as_ref().and_then(ctx_from_props),
                slot_processing: slots.as_ref().and_then(slots_processing),
                left_by_aethera: None,
            });
            break;
        }
        let mut g = self.lock();
        if g.current.is_some() {
            return;
        }
        if let Some(o) = found.as_mut() {
            o.left_by_aethera = g
                .last
                .as_ref()
                .filter(|f| f.left_running && f.run.base_url.ends_with(&format!(":{}", o.port)))
                .map(|f| f.run.run_id.clone());
        }
        g.orphan = found;
    }

    /// Termina l'orfano visto dall'ultima scansione, se è ancora lui e non sta lavorando.
    pub fn terminate_orphan(&self, pid: u32) -> Result<(), String> {
        let orphan = self.lock().orphan.clone().filter(|o| o.pid == pid).ok_or("nessun orfano con questo PID")?;
        let probe = system::probe();
        if probe.listening_pid(orphan.port) != Some(pid) {
            return Err(format!("il PID {pid} non ascolta più su :{}", orphan.port));
        }
        let slots = http_json(&http_agent(), &format!("{}/slots", orphan.base_url));
        if slots.as_ref().and_then(slots_processing) == Some(true) {
            return Err("l'orfano ha uno slot al lavoro: un client lo sta usando, terminazione rifiutata".into());
        }
        probe.terminate(pid)?;
        self.lock().orphan = None;
        Ok(())
    }
}

/// Ultime righe di un file di log, leggendo solo la coda.
pub fn read_tail(path: &Path, max_lines: usize) -> Result<String, String> {
    let mut f = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let len = f.metadata().map_err(|e| e.to_string())?.len();
    let start = len.saturating_sub(128 * 1024);
    f.seek(SeekFrom::Start(start)).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&buf);
    let mut lines: Vec<&str> = text.lines().collect();
    if start > 0 && !lines.is_empty() {
        lines.remove(0);
    }
    let from = lines.len().saturating_sub(max_lines);
    Ok(lines[from..].join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_llama_server_version() {
        let v = parse_version("version: 0.4.0-dev (build 10809, commit 5266f24da)\nbuilt with Clang 20.1.8 for Windows x86_64\n");
        assert_eq!(v.build.as_deref(), Some("b10809"));
        assert_eq!(v.commit.as_deref(), Some("5266f24da"));
        assert!(parse_version("garbage").build.is_none());
    }

    #[test]
    fn a_fork_build_is_checked_through_its_provenance() {
        let bin = Path::new(r"X:\builds\llama-b10991+moro1-win-vulkan-x64\llama-server.exe");
        let v = parse_version("version: 0.4.1-dev (build 10991, commit 8253abef6)");
        let prov = |serie: u32| -> Result<provenance::Provenance, String> {
            Ok(provenance::Provenance {
                schema_version: 1,
                id: format!("b10991+moro{serie}-vulkan"),
                base: "b10991".into(),
                commit_base: "930e2fa59".into(),
                serie,
                backend: "vulkan".into(),
                commit: "8253abef6".into(),
                data: None,
                durata_build_s: None,
                compilatore: None,
                patch: Vec::new(),
            })
        };
        // ggml-org: basta --version.
        assert!(check_build(bin, "b10991", &v, None).is_ok());
        assert!(check_build(bin, "b10809", &v, None).unwrap_err().contains("è la build b10991"));
        // Fork: tag da --version, serie dalla provenienza.
        assert!(check_build(bin, "b10991+moro1", &v, Some(&prov(1))).is_ok());
        assert!(check_build(bin, "b10991+moro2", &v, Some(&prov(1))).unwrap_err().contains("b10991+moro1"));
        assert!(check_build(bin, "b10809+moro1", &v, Some(&prov(1))).is_err());
        assert!(check_build(bin, "b10991+moro1", &v, None).unwrap_err().contains("provenienza.toml"));
        // Una build del fork non passa per quella di ggml-org, né una provenienza rotta per qualcosa.
        assert!(check_build(bin, "b10991", &v, Some(&prov(0))).unwrap_err().contains("va chiesta per nome"));
        assert!(check_build(bin, "b10991+moro1", &v, Some(&Err("rotto".into()))).is_err());
        assert!(check_build(bin, "b10991", &v, Some(&Err("rotto".into()))).is_err());
    }

    #[test]
    fn protection_and_locks_without_engine() {
        let e = Engine::default();
        assert!(!e.usage().in_use);
        e.set_protected(true);
        let u = e.usage();
        assert!(u.in_use && u.protected);
        assert_eq!(e.stop().unwrap_err(), "nessun motore acceso");
        assert!(e.acquire_lock(LockRequest { client: "nonio".into(), label: None, ttl_s: None }).is_err());
        assert_eq!(e.release_lock(None, Some("nonio")), 0);
    }

    #[test]
    fn slots_processing_reads_every_slot() {
        let v: Value = serde_json::from_str(r#"[{"id":0,"is_processing":false},{"id":1,"is_processing":true}]"#).unwrap();
        assert_eq!(slots_processing(&v), Some(true));
        assert_eq!(slots_processing(&Value::Null), None);
    }
}
