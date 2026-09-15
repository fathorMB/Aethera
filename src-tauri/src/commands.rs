//! Comandi chiamati dalla finestra.

use crate::cmdline::{self, ArgGroup};
use crate::engine::{self, EngineStatus, RunInfo, StartRequest};
use crate::import;
use crate::launch;
use crate::machine::{self, BuildEntry, DataRoot, MachineConfig, ResolvedBuild};
use crate::profile::{self, Issue, Override, Profile};
use crate::settings::ExitBehavior;
use crate::system::{self, SystemReport};
use crate::AppState;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
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

#[tauri::command]
pub fn list_profiles(state: State<AppState>) -> Result<Vec<ProfileEntry>, String> {
    let dir = data_root(&state)?.profiles();
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

#[tauri::command]
pub fn engine_start(state: State<AppState>, base: String, edited: Profile) -> Result<RunInfo, String> {
    let (root, m) = root_and_machine(&state)?;
    let run_id = chrono::Local::now().format("r-%Y%m%d-%H%M%S").to_string();
    let run_dir = root.runs().join(&run_id);
    let p = launch::prepare(&root, &m, &base, &edited, &run_dir.join("slots"));
    if !p.blockers.is_empty() {
        return Err(p.blockers.join("\n"));
    }
    state.engine.start(StartRequest {
        run_id,
        run_dir,
        profile: edited,
        profile_file: p.base_file,
        overrides: p.overrides,
        invalidates_cache: p.invalidates_cache,
        build: p.build.expect("build risolta"),
        model_path: p.model_path.expect("pesi risolti"),
        model_size: p.model_size.unwrap_or(0),
        args: p.args,
        machine_name: m.name,
        system: system::probe().report(),
    })
}

#[tauri::command]
pub fn engine_stop(state: State<AppState>) -> Result<(), String> {
    state.engine.stop()
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

/// Risposta al dialogo «Esci» quando il motore è acceso.
#[tauri::command]
pub fn app_exit(app: AppHandle, state: State<AppState>, stop: bool, remember: bool) -> Result<(), String> {
    if remember {
        let mut s = state.settings();
        s.exit_behavior = if stop { ExitBehavior::Stop } else { ExitBehavior::Leave };
        s.save()?;
    }
    if stop {
        let _ = state.engine.stop();
    } else {
        state.engine.detach();
    }
    app.exit(0);
    Ok(())
}
