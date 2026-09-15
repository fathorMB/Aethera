//! Ciclo di vita di un processo `llama-server`: avvio in job object, stato da `/health` e `/props`,
//! arresto. Un motore alla volta; nessun riavvio implicito.

use crate::cmdline;
use crate::job::Job;
use crate::machine::ResolvedBuild;
use crate::manifest::{
    self, CommandSection, EngineSection, ExitSection, MachineSection, Manifest, ModelSection, RunSection, ServerSection,
};
use crate::profile::{Override, Profile};
use crate::system::SystemReport;
use serde::Serialize;
use serde_json::Value;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct StartRequest {
    pub run_id: String,
    pub run_dir: PathBuf,
    pub profile: Profile,
    pub profile_file: Option<PathBuf>,
    pub overrides: Vec<Override>,
    pub invalidates_cache: bool,
    pub build: ResolvedBuild,
    pub model_path: PathBuf,
    pub model_size: u64,
    pub args: Vec<String>,
    pub machine_name: String,
    pub system: SystemReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunInfo {
    pub run_id: String,
    pub profile: String,
    pub base_url: String,
    pub started_at: String,
    pub pid: u32,
    pub build: String,
    pub command_line: String,
    pub log_path: String,
    pub manifest_path: String,
    pub ctx_declared: u32,
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
    },
    Ready {
        run: RunInfo,
        uptime_s: u64,
        load_ms: u64,
        ctx_served: Option<u32>,
        alias_served: Option<String>,
        divergences: Vec<String>,
    },
    Exited {
        finished: Finished,
    },
}

enum Phase {
    Loading { health: Option<u16> },
    Ready { load_ms: u64, ctx_served: Option<u32>, alias_served: Option<String> },
}

struct Running {
    child: Child,
    job: Job,
    info: RunInfo,
    started: Instant,
    phase: Phase,
    manifest: Manifest,
    manifest_path: PathBuf,
}

#[derive(Default)]
struct Inner {
    current: Option<Running>,
    last: Option<Finished>,
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

fn http_json(agent: &ureq::Agent, url: &str) -> Option<Value> {
    let mut r = agent.get(url).call().ok()?;
    if r.status().as_u16() != 200 {
        return None;
    }
    r.body_mut().read_json::<Value>().ok()
}

fn divergences(info: &RunInfo, ctx_served: Option<u32>, alias_served: Option<&str>) -> Vec<String> {
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
    d
}

fn now() -> String {
    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
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

impl Engine {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn is_running(&self) -> bool {
        self.lock().current.is_some()
    }

    pub fn status(&self) -> EngineStatus {
        let g = self.lock();
        match &g.current {
            Some(r) => match &r.phase {
                Phase::Loading { health } => {
                    EngineStatus::Loading { run: r.info.clone(), elapsed_s: r.started.elapsed().as_secs(), health: *health }
                }
                Phase::Ready { load_ms, ctx_served, alias_served } => EngineStatus::Ready {
                    run: r.info.clone(),
                    uptime_s: r.started.elapsed().as_secs(),
                    load_ms: *load_ms,
                    ctx_served: *ctx_served,
                    alias_served: alias_served.clone(),
                    divergences: divergences(&r.info, *ctx_served, alias_served.as_deref()),
                },
            },
            None => match &g.last {
                Some(f) if !f.by_user => EngineStatus::Exited { finished: f.clone() },
                other => EngineStatus::Off { last: other.clone() },
            },
        }
    }

    pub fn log_path(&self) -> Option<PathBuf> {
        let g = self.lock();
        g.current
            .as_ref()
            .map(|r| r.info.log_path.clone())
            .or_else(|| g.last.as_ref().map(|f| f.run.log_path.clone()))
            .map(PathBuf::from)
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
        match &version.build {
            Some(b) if *b == req.profile.runtime.build => {}
            Some(b) => {
                return Err(format!(
                    "{} è la build {b}, il profilo dichiara {}",
                    req.build.binary.display(),
                    req.profile.runtime.build
                ))
            }
            None => return Err(format!("build non leggibile da --version: {}", version.text)),
        }

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
        let info = RunInfo {
            run_id: req.run_id.clone(),
            profile: req.profile.name.clone(),
            base_url: format!("http://{}:{}", s.host, s.port),
            started_at: started_at.clone(),
            pid: child.id(),
            build: req.profile.runtime.build.clone(),
            command_line: command_line.clone(),
            log_path: log_path.display().to_string(),
            manifest_path: manifest_path.display().to_string(),
            ctx_declared: s.ctx,
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
                ram_available_gib_before: req.system.ram_available_gib,
            },
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
            g.current = Some(Running {
                child,
                job,
                info: info.clone(),
                started,
                phase: Phase::Loading { health: None },
                manifest,
                manifest_path,
            });
        }
        let engine = self.clone();
        let run_id = info.run_id.clone();
        std::thread::spawn(move || engine.monitor(run_id));
        Ok(info)
    }

    /// Controlla l'uscita del processo e, finché carica, `/health`; a 200 legge `/props` una volta.
    fn monitor(&self, run_id: String) {
        let agent = http_agent();
        loop {
            std::thread::sleep(Duration::from_millis(500));
            let (base_url, loading) = {
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
                (r.info.base_url.clone(), matches!(r.phase, Phase::Loading { .. }))
            };
            if !loading {
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
            let ctx_served = props
                .as_ref()
                .and_then(|v| v["default_generation_settings"]["n_ctx"].as_u64().or_else(|| v["n_ctx"].as_u64()))
                .map(|n| n as u32);
            let alias_served = props.as_ref().and_then(|v| v["model_alias"].as_str().map(str::to_string));
            let load_ms = r.started.elapsed().as_millis() as u64;
            r.manifest.server.ctx_served = ctx_served;
            r.manifest.server.alias_served = alias_served.clone();
            r.manifest.server.load_ms = Some(load_ms);
            r.manifest.server.ready_at = Some(now());
            let _ = manifest::write(&r.manifest_path, &r.manifest);
            r.phase = Phase::Ready { load_ms, ctx_served, alias_served };
        }
    }

    pub fn stop(&self) -> Result<(), String> {
        let mut g = self.lock();
        let Some(mut r) = g.current.take() else { return Err("nessun motore acceso".into()) };
        r.job.terminate();
        let _ = r.child.kill();
        let code = r.child.wait().ok().and_then(|s| s.code());
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
}
