//! Comandi chiamati dalla finestra (e dalla tray).

use crate::clients::{self, ClientSnippets, RunFacts};
use crate::cmdline::{self, ArgGroup};
use crate::engine::{self, EngineStatus, RunInfo, StartRequest, Usage};
use crate::import;
use crate::launch;
use crate::machine::{self, BuildEntry, DataRoot, MachineConfig, ResolvedBuild};
use crate::profile::{self, Issue, Override, Profile};
use crate::runs::{self, Comparison, RunDetail, RunRow};
use crate::settings::ExitBehavior;
use crate::system::{self, SystemReport};
use crate::AppState;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tauri::{AppHandle, State};

#[derive(Serialize)]
pub struct Overview {
    pub data_root: Option<String>,
    pub machine: Option<MachineConfig>,
    pub machine_error: Option<String>,
    pub system: SystemReport,
    pub exit_behavior: ExitBehavior,
    pub builds: Vec<ResolvedBuild>,
    pub builds_missing: Vec<BuildEntry>,
    pub endpoint: Option<String>,
    pub endpoint_error: Option<String>,
}

fn data_root(state: &AppState) -> Result<DataRoot, String> {
    state.settings().data_root.clone().map(DataRoot::new).ok_or_else(|| "radice dati non ancora scelta".to_string())
}

fn root_and_machine(state: &AppState) -> Result<(DataRoot, MachineConfig), String> {
    let root = data_root(state)?;
    let machine = root.load_machine()?;
    Ok((root, machine))
}

fn build_overview(state: &AppState) -> Overview {
    let settings = state.settings().clone();
    let mut o = Overview {
        data_root: settings.data_root.as_ref().map(|p| p.display().to_string()),
        machine: None,
        machine_error: None,
        system: system::probe().report(),
        exit_behavior: settings.exit_behavior,
        builds: Vec::new(),
        builds_missing: Vec::new(),
        endpoint: state.endpoint.as_ref().ok().cloned(),
        endpoint_error: state.endpoint.as_ref().err().cloned(),
    };
    if let Some(path) = settings.data_root {
        let root = DataRoot::new(path);
        match root.ensure(&o.system) {
            Ok(m) => {
                o.builds = root.builds_available(&m);
                o.builds_missing =
                    m.builds.iter().filter(|b| !b.path.join(machine::server_binary_name()).is_file()).cloned().collect();
                o.machine = Some(m);
            }
            Err(e) => o.machine_error = Some(e),
        }
    }
    o
}

#[tauri::command]
pub fn overview(state: State<AppState>) -> Overview {
    build_overview(&state)
}

#[tauri::command]
pub fn set_data_root(state: State<AppState>, path: String) -> Result<Overview, String> {
    let path = PathBuf::from(path.trim());
    if !path.is_absolute() {
        return Err("serve un percorso assoluto".into());
    }
    DataRoot::new(&path).ensure(&system::probe().report())?;
    {
        let mut s = state.settings();
        s.data_root = Some(path);
        s.save()?;
    }
    Ok(build_overview(&state))
}

#[tauri::command]
pub fn save_machine(state: State<AppState>, machine: MachineConfig) -> Result<Overview, String> {
    if machine.name.trim().is_empty() {
        return Err("il nome della macchina è obbligatorio".into());
    }
    data_root(&state)?.save_machine(&machine)?;
    Ok(build_overview(&state))
}

#[tauri::command]
pub fn set_exit_behavior(state: State<AppState>, behavior: ExitBehavior) -> Result<(), String> {
    let mut s = state.settings();
    s.exit_behavior = behavior;
    s.save()
}

#[derive(Serialize)]
pub struct ProfileEntry {
    pub name: String,
    pub file: String,
    pub profile: Option<Profile>,
    pub issues: Vec<Issue>,
}

fn read_profiles(root: &DataRoot) -> Result<Vec<ProfileEntry>, String> {
    let dir = root.profiles();
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    files.sort();
    Ok(files
        .into_iter()
        .map(|path| {
            let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let (profile, issues) = match fs::read_to_string(&path) {
                Ok(text) => profile::load(&text, &stem),
                Err(e) => (None, vec![Issue { field: String::new(), message: e.to_string() }]),
            };
            ProfileEntry { name: stem, file: path.display().to_string(), profile, issues }
        })
        .collect())
}

#[tauri::command]
pub fn list_profiles(state: State<AppState>) -> Result<Vec<ProfileEntry>, String> {
    read_profiles(&data_root(&state)?)
}

/// Host e porte dichiarati dai profili: dove può stare un `llama-server` orfano.
pub fn profile_endpoints(state: &AppState) -> Vec<(String, u16)> {
    let mut out: Vec<(String, u16)> = data_root(state)
        .and_then(|r| read_profiles(&r))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|e| e.profile.map(|p| (p.server.host, p.server.port)))
        .collect();
    out.sort();
    out.dedup();
    out
}

#[derive(Serialize)]
pub struct Preview {
    pub issues: Vec<Issue>,
    pub overrides: Vec<Override>,
    pub invalidates_cache: bool,
    pub blockers: Vec<String>,
    pub args: Vec<ArgGroup>,
    pub line: String,
    pub binary: Option<String>,
    pub model_path: Option<String>,
    pub model_size_gb: Option<f64>,
}

#[tauri::command]
pub fn preview(state: State<AppState>, base: String, edited: Profile) -> Result<Preview, String> {
    let (root, m) = root_and_machine(&state)?;
    let p = launch::prepare(&root, &m, &base, &edited, &root.runs().join("{run}").join("slots"));
    let binary = p.build.as_ref().map(|b| b.binary.clone()).unwrap_or_else(|| PathBuf::from(machine::server_binary_name()));
    Ok(Preview {
        args: cmdline::compare(p.base_args.as_deref().unwrap_or(&p.args), &p.args),
        line: cmdline::render_line(&binary, &p.args),
        binary: p.build.map(|b| b.binary.display().to_string()),
        model_path: p.model_path.map(|x| x.display().to_string()),
        model_size_gb: p.model_size.map(|b| (b as f64 / 1e7).round() / 100.0),
        issues: p.issues,
        overrides: p.overrides,
        invalidates_cache: p.invalidates_cache,
        blockers: p.blockers,
    })
}

#[tauri::command]
pub fn save_profile(state: State<AppState>, profile: Profile, replace: Option<String>) -> Result<String, String> {
    let issues = profile::validate(&profile);
    if !issues.is_empty() {
        return Err(issues.iter().map(|i| format!("{}: {}", i.field, i.message)).collect::<Vec<_>>().join("\n"));
    }
    let path = data_root(&state)?.profiles().join(format!("{}.toml", profile.name));
    if path.exists() && replace.as_deref() != Some(profile.name.as_str()) {
        return Err(format!("esiste già un profilo «{}»: scegli un altro nome", profile.name));
    }
    let text = toml::to_string_pretty(&profile).map_err(|e| e.to_string())?;
    fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(profile.name)
}

#[derive(Serialize)]
pub struct ImportReport {
    pub name: String,
    pub notes: Vec<String>,
}

#[tauri::command]
pub fn import_minis(state: State<AppState>, path: String, build: String) -> Result<ImportReport, String> {
    let root = data_root(&state)?;
    let source = PathBuf::from(&path);
    let text = fs::read_to_string(&source).map_err(|e| format!("{path}: {e}"))?;
    let label = source.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(path.clone());
    let imported = import::import_minis_json(&text, &label, build.trim())?;
    let issues = profile::validate(&imported.profile);
    if !issues.is_empty() {
        return Err(issues.iter().map(|i| format!("{}: {}", i.field, i.message)).collect::<Vec<_>>().join("\n"));
    }
    let target = root.profiles().join(format!("{}.toml", imported.profile.name));
    if target.exists() {
        return Err(format!("esiste già {}: non lo sovrascrivo", target.display()));
    }
    let toml_text = toml::to_string_pretty(&imported.profile).map_err(|e| e.to_string())?;
    fs::write(&target, toml_text).map_err(|e| format!("{}: {e}", target.display()))?;
    Ok(ImportReport { name: imported.profile.name, notes: imported.notes })
}

fn start_from(state: &AppState, base: &str, edited: Profile) -> Result<RunInfo, String> {
    let (root, m) = root_and_machine(state)?;
    let run_id = chrono::Local::now().format("r-%Y%m%d-%H%M%S").to_string();
    let run_dir = root.runs().join(&run_id);
    let p = launch::prepare(&root, &m, base, &edited, &run_dir.join("slots"));
    if !p.blockers.is_empty() {
        return Err(p.blockers.join("\n"));
    }
    state.engine.start(StartRequest {
        run_id,
        run_dir,
        base: base.to_string(),
        profile: edited,
        profile_file: p.base_file,
        overrides: p.overrides,
        invalidates_cache: p.invalidates_cache,
        build: p.build.expect("build risolta"),
        model_path: p.model_path.expect("pesi risolti"),
        model_size: p.model_size.unwrap_or(0),
        args: p.args,
        machine_name: m.name,
        ram_margin_gib: m.ram_margin_gib,
        system: system::probe().report(),
    })
}

#[tauri::command]
pub fn engine_start(state: State<AppState>, base: String, edited: Profile) -> Result<RunInfo, String> {
    start_from(&state, &base, edited)
}

/// «Riavvia con la stessa riga»: stesso profilo e stesse differenze, nuovo avvio. Rifiutato se in uso.
pub fn restart(state: &AppState) -> Result<RunInfo, String> {
    let (base, profile) = state.engine.source().ok_or("nessun avvio da ripetere in questa sessione")?;
    if state.engine.is_running() {
        state.engine.stop()?;
    }
    // La porta si libera qualche istante dopo la fine del processo.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match start_from(state, &base, profile.clone()) {
            Err(e) if e.contains("risponde già") && Instant::now() < deadline => std::thread::sleep(Duration::from_millis(250)),
            other => return other,
        }
    }
}

#[tauri::command]
pub fn engine_restart(state: State<AppState>) -> Result<RunInfo, String> {
    restart(&state)
}

#[tauri::command]
pub fn engine_stop(state: State<AppState>) -> Result<(), String> {
    state.engine.stop()
}

#[tauri::command]
pub fn engine_protect(state: State<AppState>, on: bool) -> Usage {
    state.engine.set_protected(on);
    state.engine.usage()
}

#[tauri::command]
pub fn engine_status(state: State<AppState>) -> EngineStatus {
    state.engine.status()
}

#[tauri::command]
pub fn engine_log(state: State<AppState>, lines: usize) -> Result<String, String> {
    match state.engine.log_path() {
        Some(p) => engine::read_tail(&p, lines.clamp(1, 2000)),
        None => Ok(String::new()),
    }
}

#[tauri::command]
pub fn orphan_terminate(state: State<AppState>, pid: u32) -> Result<(), String> {
    state.engine.terminate_orphan(pid)
}

pub fn snippets_for(state: &AppState) -> Option<ClientSnippets> {
    let m = state.engine.manifest()?;
    let base_url = format!("http://{}:{}", m.server.host, m.server.port);
    Some(clients::snippets(&RunFacts {
        run_id: &m.run.id,
        base_url: &base_url,
        alias: &m.server.alias,
        ctx_declared: m.server.ctx_declared,
        ctx_served: m.server.ctx_served,
        client: m.effective_profile.client.as_ref(),
        endpoint: state.endpoint.as_ref().ok().map(String::as_str),
    }))
}

#[tauri::command]
pub fn client_snippets(state: State<AppState>) -> Option<ClientSnippets> {
    snippets_for(&state)
}

fn current_run(state: &AppState) -> Option<String> {
    state.engine.manifest().map(|m| m.run.id)
}

#[tauri::command]
pub fn runs_list(state: State<AppState>) -> Result<Vec<RunRow>, String> {
    Ok(runs::list(&data_root(&state)?.runs(), current_run(&state).as_deref()))
}

#[tauri::command]
pub fn run_detail(state: State<AppState>, id: String) -> Result<RunDetail, String> {
    runs::detail(&data_root(&state)?.runs(), &id, current_run(&state).as_deref())
}

#[tauri::command]
pub fn runs_compare(state: State<AppState>, a: String, b: String) -> Result<Comparison, String> {
    runs::compare(&data_root(&state)?.runs(), &a, &b, current_run(&state).as_deref())
}

/// Risposta al dialogo «Esci» quando il motore è acceso.
#[tauri::command]
pub fn app_exit(app: AppHandle, state: State<AppState>, stop: bool, remember: bool) -> Result<(), String> {
    if stop {
        // Un motore in uso non si ferma: l'errore torna al dialogo e Aethera resta aperta.
        if state.engine.is_running() {
            state.engine.stop()?;
        }
    } else {
        state.engine.detach();
    }
    if remember {
        let mut s = state.settings();
        s.exit_behavior = if stop { ExitBehavior::Stop } else { ExitBehavior::Leave };
        s.save()?;
    }
    app.exit(0);
    Ok(())
}
