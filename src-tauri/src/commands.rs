//! Comandi chiamati dalla finestra (e dalla tray).

use crate::adopt;
use crate::builds::{self, Release};
use crate::catalog::{self, Catalog, ModelRow, ScanInput};
use crate::clients::{self, ClientSnippets, RunFacts};
use crate::cmdline::{self, ArgGroup};
use crate::download::{self, Request as DownloadRequest};
use crate::engine::{self, EngineStatus, RunInfo, StartRequest, Usage};
use crate::estimate::{self, Estimate};
use crate::gguf::ModelInfo;
use crate::hash;
use crate::import;
use crate::launch;
use crate::machine::{self, BuildEntry, DataRoot, MachineConfig, ResolvedBuild};
use crate::modelcard;
use crate::profile::{self, Issue, Override, Profile};
use crate::runs::{self, Comparison, RunDetail, RunRow};
use crate::settings::ExitBehavior;
use crate::system::{self, SystemReport};
use crate::tasks::{Kind, TaskView};
use crate::AppState;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, State};

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
    /// Stima di memoria alle leve correnti: si aggiorna mentre si cambiano contesto e cache.
    pub estimate: Option<Estimate>,
}

/// Stima per l'anteprima. Usa **solo** i metadati GGUF già in catalogo: leggerli dal file costa
/// oltre un secondo su 22 GB e l'anteprima si aggiorna a ogni tasto. Se il catalogo non li ha
/// ancora (basta aprire la pagina Catalogo), restano note i soli pesi e il totale è un minimo.
fn estimate_for(root: &DataRoot, p: &Profile, model_size: Option<u64>) -> Estimate {
    let cat = catalog::load(root).unwrap_or_default();
    let known: BTreeMap<String, ModelInfo> =
        cat.models.iter().filter_map(|e| e.gguf.as_ref().map(|g| (e.file.clone(), g.info.clone()))).collect();
    let info = known.get(&p.model.file).cloned();
    let compute = catalog::measured_compute(&root.runs(), &|file| known.get(file).cloned());
    let measured = compute.get(&catalog::compute_key(p.server.ubatch, &p.runtime.backend)).cloned();
    estimate::estimate(info.as_ref(), &p.server, model_size, measured)
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
        estimate: Some(estimate_for(&root, &edited, p.model_size)),
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

// ---------------------------------------------------------------------------------------------
// Profili dalla finestra (M-05): crearne uno da zero, duplicarlo, rinominarlo, cancellarlo.
//
// Il nome del profilo è il nome del file **ed è l'alias servito**: rinominare vuol dire spostare
// il file e riscrivere il campo `name`, non una cosa sola delle due. Gli avvii già registrati
// citano il profilo per nome nel loro manifest e non si toccano: un manifest racconta com'è
// andato quell'avvio, non punta a un file che deve ancora esistere.
// ---------------------------------------------------------------------------------------------

fn profile_file(root: &DataRoot, name: &str) -> PathBuf {
    root.profiles().join(format!("{name}.toml"))
}

/// Prima porta non dichiarata da nessun profilo, dalla 8080 in su.
fn free_port(taken: &[u16]) -> u16 {
    (8080..8180).find(|p| !taken.contains(p)).unwrap_or(8080)
}

/// Nome libero: `nuovo-profilo`, poi `nuovo-profilo-2`, `nuovo-profilo-3`…
fn unique_name(root: &DataRoot, wanted: &str) -> String {
    if !profile_file(root, wanted).exists() {
        return wanted.to_string();
    }
    (2..1000)
        .map(|n| format!("{wanted}-{n}"))
        .find(|n| !profile_file(root, n).exists())
        .unwrap_or_else(|| wanted.to_string())
}

/// Il profilo da cui è stato acceso il motore che sta girando: non si rinomina né si cancella
/// sotto i piedi di un motore acceso, perché «Riavvia» tornerebbe a cercarlo.
fn refuse_if_running(state: &AppState, name: &str) -> Result<(), String> {
    let Some(m) = state.engine.manifest() else { return Ok(()) };
    if state.engine.is_running() && m.run.profile == name {
        return Err(format!("il motore acceso ({}) è partito da questo profilo: fermalo prima", m.run.id));
    }
    Ok(())
}

fn read_profile(root: &DataRoot, name: &str) -> Result<Profile, String> {
    let path = profile_file(root, name);
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (p, issues) = profile::load(&text, name);
    p.ok_or_else(|| {
        format!("{}: {}", path.display(), issues.first().map(|i| i.message.clone()).unwrap_or_else(|| "illeggibile".into()))
    })
}

fn write_profile(root: &DataRoot, p: &Profile) -> Result<(), String> {
    let issues = profile::validate(p);
    if !issues.is_empty() {
        return Err(issues.iter().map(|i| format!("{}: {}", i.field, i.message)).collect::<Vec<_>>().join("\n"));
    }
    let path = profile_file(root, &p.name);
    let text = toml::to_string_pretty(p).map_err(|e| e.to_string())?;
    fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Un profilo nuovo con valori sensati, già pieno di quello che la macchina e il catalogo sanno.
/// Non scrive niente: la finestra lo mostra, lo si corregge e si salva.
#[tauri::command]
pub fn profile_template(state: State<AppState>, model_file: Option<String>) -> Result<Profile, String> {
    let (root, machine) = root_and_machine(&state)?;
    let taken: Vec<u16> =
        read_profiles(&root)?.iter().filter_map(|e| e.profile.as_ref().map(|p| p.server.port)).collect();

    // La build proposta è la prima installata: fissarla è una scelta dichiarata, non un automatismo.
    let builds = root.builds_available(&machine);
    let (build, backend) = builds.first().map(|b| machine::split_build_id(&b.id)).unwrap_or((None, None));

    let file = model_file.unwrap_or_default().trim().to_string();
    let wanted = if file.is_empty() { "nuovo-profilo".to_string() } else { profile::name_from_file(&file) };
    let mut p = profile::template(
        &unique_name(&root, &wanted),
        &file,
        build.as_deref().unwrap_or_default(),
        backend.as_deref().unwrap_or("vulkan"),
        free_port(&taken),
    );

    // Quello che il catalogo sa già del modello non si chiede una seconda volta a chi crea il profilo.
    if !file.is_empty() {
        if let Some(e) = catalog::load(&root)?.models.into_iter().find(|e| e.file.eq_ignore_ascii_case(&file)) {
            p.model.file = e.file;
            p.model.repo = e.repo;
            p.model.quant = e.quant.or_else(|| e.gguf.as_ref().and_then(|g| g.info.dominant_type.clone()));
            p.model.size_gb = e.size_gb;
            p.model.sha256 = e.sha256_verified.or(e.sha256);
            p.sampling_by_mode = e.sampling_by_mode;
        }
    }
    Ok(p)
}

/// Copia un profilo con un altro nome. L'alias segue il nome: due profili con lo stesso alias
/// servirebbero lo stesso modello sotto lo stesso nome e un client non li distinguerebbe.
#[tauri::command]
pub fn profile_duplicate(state: State<AppState>, name: String, new_name: String) -> Result<String, String> {
    let root = data_root(&state)?;
    let wanted = new_name.trim().to_string();
    if wanted == name {
        return Err("il nome nuovo è uguale a quello vecchio".into());
    }
    if profile_file(&root, &wanted).exists() {
        return Err(format!("esiste già un profilo «{wanted}»: scegli un altro nome"));
    }
    let mut p = read_profile(&root, &name)?;
    p.name = wanted.clone();
    write_profile(&root, &p)?;
    Ok(wanted)
}

/// Rinomina un profilo: file e campo `name` insieme, e il vecchio file sparisce solo quando il
/// nuovo è stato scritto davvero.
#[tauri::command]
pub fn profile_rename(state: State<AppState>, name: String, new_name: String) -> Result<String, String> {
    let root = data_root(&state)?;
    let wanted = new_name.trim().to_string();
    if wanted == name {
        return Ok(name);
    }
    refuse_if_running(&state, &name)?;
    if profile_file(&root, &wanted).exists() {
        return Err(format!("esiste già un profilo «{wanted}»: scegli un altro nome"));
    }
    let mut p = read_profile(&root, &name)?;
    p.name = wanted.clone();
    write_profile(&root, &p)?;
    let old = profile_file(&root, &name);
    fs::remove_file(&old).map_err(|e| format!("{}: {e} (il profilo «{wanted}» è stato scritto)", old.display()))?;
    Ok(wanted)
}

#[derive(Serialize)]
pub struct DeletionPlan {
    pub name: String,
    pub file: String,
    /// Avvii già registrati che citano questo profilo: restano dove sono.
    pub runs: usize,
    pub warnings: Vec<String>,
}

/// Che cosa comporta cancellare un profilo: lo si dice prima, non dopo.
#[tauri::command]
pub fn profile_deletion_plan(state: State<AppState>, name: String) -> Result<DeletionPlan, String> {
    let root = data_root(&state)?;
    let file = profile_file(&root, &name);
    if !file.is_file() {
        return Err(format!("nessun profilo «{name}»"));
    }
    let runs = runs::list(&root.runs(), None).into_iter().filter(|r| r.profile == name).count();
    let mut warnings = Vec::new();
    if let Err(e) = refuse_if_running(&state, &name) {
        warnings.push(e);
    }
    if runs > 0 {
        warnings.push(format!(
            "{runs} avvii registrati citano «{name}»: restano nello storico con il loro profilo effettivo, ma il file da cui sono partiti non ci sarà più"
        ));
    }
    Ok(DeletionPlan { name, file: file.display().to_string(), runs, warnings })
}

/// Cancella il file del profilo. Gli avvii già registrati non si toccano: il manifest di ognuno
/// porta dentro il profilo effettivo, quindi resta leggibile e confrontabile anche senza il file.
#[tauri::command]
pub fn profile_delete(state: State<AppState>, name: String) -> Result<(), String> {
    let root = data_root(&state)?;
    refuse_if_running(&state, &name)?;
    let file = profile_file(&root, &name);
    if !file.is_file() {
        return Err(format!("nessun profilo «{name}»"));
    }
    fs::remove_file(&file).map_err(|e| format!("{}: {e}", file.display()))
}

/// Campionamento consigliato che il catalogo conosce per i pesi di un profilo: si porta nel
/// profilo con un gesto, invece di ricopiarlo a mano dalla pagina Catalogo.
#[tauri::command]
pub fn catalog_sampling(state: State<AppState>, file: String) -> Result<BTreeMap<String, profile::Sampling>, String> {
    let root = data_root(&state)?;
    Ok(catalog::load(&root)?
        .models
        .into_iter()
        .find(|e| e.file.eq_ignore_ascii_case(file.trim()))
        .map(|e| e.sampling_by_mode)
        .unwrap_or_default())
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
        // Il campionamento è quello del profilo **avviato**, non quello del file su disco:
        // le righe devono valere per il motore che sta rispondendo adesso.
        sampling: &m.effective_profile.sampling_by_mode,
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

// ---------------------------------------------------------------------------------------------
// Catalogo (M-04): modelli, verifica dell'hash, download, build.
// I lavori lunghi non si fanno qui dentro: partono in un thread e lasciano l'avanzamento in `tasks`.
// ---------------------------------------------------------------------------------------------

/// Due percorsi che puntano allo stesso file, anche scritti in modo diverso.
fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

/// I pesi che il motore acceso sta usando non si toccano: né verifica, né download sopra, né rimozione.
fn refuse_if_in_use(state: &AppState, path: &Path) -> Result<(), String> {
    let Some(m) = state.engine.manifest() else { return Ok(()) };
    if same_file(Path::new(&m.model.path), path) {
        return Err(format!(
            "il motore acceso ({}) sta usando questi pesi: fermalo prima di toccare il file",
            m.run.id
        ));
    }
    Ok(())
}

fn profiles_of(root: &DataRoot) -> Vec<Profile> {
    read_profiles(root).unwrap_or_default().into_iter().filter_map(|e| e.profile).collect()
}

/// Scansione completa: legge il catalogo, aggiorna i metadati GGUF, ricava i buffer misurati
/// dagli avvii e li usa per la stima. Salva il catalogo se la lettura l'ha arricchito.
fn scan_catalog(state: &AppState) -> Result<(Catalog, Vec<ModelRow>), String> {
    let (root, machine) = root_and_machine(state)?;
    let profiles = profiles_of(&root);
    let probe = system::probe();
    let mut cat = catalog::load(&root)?;
    let before = cat.clone();

    // Primo giro senza buffer: serve a riempire la cache dei metadati GGUF…
    let empty = BTreeMap::new();
    let input = ScanInput { machine: &machine, profiles: &profiles, compute: &empty, probe: probe.as_ref() };
    catalog::scan(&mut cat, &input);
    // …che poi permette di calcolare la KV degli avvii passati e quindi il buffer misurato.
    let known: BTreeMap<String, ModelInfo> =
        cat.models.iter().filter_map(|e| e.gguf.as_ref().map(|g| (e.file.clone(), g.info.clone()))).collect();
    let compute = catalog::measured_compute(&root.runs(), &|file| known.get(file).cloned());
    let input = ScanInput { machine: &machine, profiles: &profiles, compute: &compute, probe: probe.as_ref() };
    let rows = catalog::scan(&mut cat, &input);

    if cat != before {
        catalog::save(&root, &cat)?;
    }
    Ok((cat, rows))
}

#[tauri::command]
pub fn catalog_list(state: State<AppState>) -> Result<Vec<ModelRow>, String> {
    Ok(scan_catalog(&state)?.1)
}

fn entry_path(state: &AppState, id: &str) -> Result<(DataRoot, catalog::Entry, PathBuf), String> {
    let (root, machine) = root_and_machine(state)?;
    let cat = catalog::load(&root)?;
    let entry = cat.models.iter().find(|e| e.id == id).cloned().ok_or_else(|| format!("nessuna voce «{id}» nel catalogo"))?;
    let dir = machine.models_dir.ok_or("cartella dei pesi non impostata (Impostazioni → machine.toml)")?;
    let path = dir.join(&entry.file);
    Ok((root, entry, path))
}

/// Aggiorna una voce sul disco sotto il lucchetto del catalogo: due lavori non si sovrascrivono.
fn update_entry(state: &AppState, id: &str, change: impl FnOnce(&mut catalog::Entry)) -> Result<(), String> {
    let _g = state.catalog_lock.lock().unwrap_or_else(|e| e.into_inner());
    let root = data_root(state)?;
    let mut cat = catalog::load(&root)?;
    let entry = cat.models.iter_mut().find(|e| e.id == id).ok_or_else(|| format!("nessuna voce «{id}»"))?;
    change(entry);
    catalog::save(&root, &cat)
}

/// Ricalcola lo SHA-256 di un file presente: minuti su 22 GB, quindi in un thread, annullabile.
#[tauri::command]
pub fn catalog_verify(app: AppHandle, state: State<AppState>, id: String) -> Result<String, String> {
    let (_, entry, path) = entry_path(&state, &id)?;
    if !path.is_file() {
        return Err(format!("{} non è sul disco: non c'è niente da verificare", entry.file));
    }
    refuse_if_in_use(&state, &path)?;
    let (task_id, cancel) = state.tasks.start(Kind::Verify, &id)?;
    let expected = entry.sha256.clone();
    let tid = task_id.clone();
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let result = run_verify(&state, &id, &path, expected.as_deref(), &tid, &cancel);
        state.tasks.finish(&tid, result);
    });
    Ok(task_id)
}

fn run_verify(
    state: &AppState,
    id: &str,
    path: &Path,
    expected: Option<&str>,
    task_id: &str,
    cancel: &AtomicBool,
) -> Result<String, String> {
    let tasks = state.tasks.clone();
    let tid = task_id.to_string();
    let out = hash::sha256_file(path, cancel, &mut |done, total| tasks.progress(&tid, done, Some(total)))?;
    let got = match out {
        crate::hash::Outcome::Cancelled => return Ok("verifica annullata: l'hash non è stato scritto".into()),
        crate::hash::Outcome::Done(h) => h,
    };
    let when = chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false);
    let verified = got.clone();
    update_entry(state, id, |e| {
        e.sha256_verified = Some(verified);
        e.verified_at = Some(when);
        // Senza hash atteso, quello appena calcolato diventa il riferimento.
        if e.sha256.is_none() {
            e.sha256 = e.sha256_verified.clone();
        }
    })?;
    match expected {
        Some(exp) if hash::matches(exp, &got) => Ok(format!("verificato: {}", hash::abbreviate(&got))),
        Some(exp) => Err(format!(
            "SHA-256 diverso: atteso {}, calcolato {}",
            hash::abbreviate(exp),
            hash::abbreviate(&got)
        )),
        None => Ok(format!("calcolato: {}", hash::abbreviate(&got))),
    }
}

/// Aggiunge al catalogo un file di un repository di Hugging Face, con oid e dimensione dichiarati.
#[tauri::command]
pub fn catalog_add(state: State<AppState>, repo: String, file: String) -> Result<Vec<ModelRow>, String> {
    let remote = download::remote_file(repo.trim(), file.trim())?;
    {
        let _g = state.catalog_lock.lock().unwrap_or_else(|e| e.into_inner());
        let root = data_root(&state)?;
        let mut cat = catalog::load(&root)?;
        if cat.models.iter().any(|e| e.file.eq_ignore_ascii_case(&remote.file)) {
            return Err(format!("«{}» è già nel catalogo", remote.file));
        }
        let mut entry = catalog::Entry::discovered(&remote.file);
        entry.repo = Some(remote.repo.clone());
        entry.sha256 = remote.oid.clone();
        entry.size_gb = remote.size.map(|b| (b as f64 / 1e7).round() / 100.0);
        cat.models.push(entry);
        catalog::save(&root, &cat)?;
    }
    Ok(scan_catalog(&state)?.1)
}

/// Scarica i pesi di una voce, riprendendo da un `.part` se c'è.
#[tauri::command]
pub fn catalog_download(app: AppHandle, state: State<AppState>, id: String) -> Result<String, String> {
    let (_, entry, path) = entry_path(&state, &id)?;
    let repo = entry.repo.clone().ok_or_else(|| format!("«{}» non dichiara un repository da cui scaricarlo", entry.id))?;
    if path.is_file() {
        return Err(format!("{} è già sul disco", entry.file));
    }
    refuse_if_in_use(&state, &path)?;
    let (task_id, cancel) = state.tasks.start(Kind::Download, &id)?;
    let tid = task_id.clone();
    let file = entry.file.clone();
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let result = run_download(&state, &id, &repo, &file, &path, &tid, &cancel);
        state.tasks.finish(&tid, result);
    });
    Ok(task_id)
}

fn run_download(
    state: &AppState,
    id: &str,
    repo: &str,
    file: &str,
    path: &Path,
    task_id: &str,
    cancel: &AtomicBool,
) -> Result<String, String> {
    let remote = download::remote_file(repo, file)?;
    let tasks = state.tasks.clone();
    let tid = task_id.to_string();
    let probe = system::probe();
    let out = download::download(
        &DownloadRequest {
            url: remote.url.clone(),
            target: path.to_path_buf(),
            expected_sha256: remote.oid.clone(),
            expected_size: remote.size,
            free_disk: probe.free_disk_bytes(path),
            cancel,
        },
        &mut |done, total| tasks.progress(&tid, done, total),
    )?;
    match out {
        download::Outcome::AlreadyThere => Ok("il file c'era già".into()),
        download::Outcome::Paused { bytes } => {
            Ok(format!("in pausa a {:.2} GB: riprendibile", bytes as f64 / 1e9))
        }
        download::Outcome::Done { bytes, sha256 } => {
            let oid = remote.oid.clone();
            update_entry(state, id, |e| {
                if e.sha256.is_none() {
                    e.sha256 = oid;
                }
                e.sha256_verified = sha256.clone();
                e.verified_at = sha256
                    .is_some()
                    .then(|| chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false));
            })?;
            Ok(format!("scaricato e verificato: {:.2} GB", bytes as f64 / 1e9))
        }
    }
}

/// Rilegge i metadati dal GGUF anche se il file non è cambiato («Leggi metadati GGUF»).
#[tauri::command]
pub fn catalog_reread(state: State<AppState>, id: String) -> Result<Vec<ModelRow>, String> {
    update_entry(&state, &id, |e| e.gguf = None)?;
    Ok(scan_catalog(&state)?.1)
}

#[derive(Serialize)]
pub struct RemovalPlan {
    pub file: String,
    pub path: Option<String>,
    pub hard_links: Option<u32>,
    /// Perché serve una conferma esplicita prima di cancellare.
    pub warnings: Vec<String>,
}

/// Che cosa comporta togliere una voce: lo si dice prima, non dopo.
#[tauri::command]
pub fn catalog_removal_plan(state: State<AppState>, id: String) -> Result<RemovalPlan, String> {
    let (_, entry, path) = entry_path(&state, &id)?;
    let probe = system::probe();
    let links = path.is_file().then(|| probe.hard_links(&path)).flatten();
    let mut warnings = Vec::new();
    if let Some(n) = links.filter(|n| *n > 1) {
        warnings.push(format!(
            "questo file ha {n} collegamenti: lo stesso contenuto è usato da un altro strumento e cancellandolo sparirebbe anche di lì"
        ));
    }
    if refuse_if_in_use(&state, &path).is_err() {
        warnings.push("il motore acceso sta usando questi pesi".into());
    }
    Ok(RemovalPlan { file: entry.file, path: path.is_file().then(|| path.display().to_string()), hard_links: links, warnings })
}

/// Toglie la voce dal catalogo e, se richiesto, cancella il file. Con più collegamenti o con il
/// motore che lo usa serve `confirm`.
#[tauri::command]
pub fn catalog_remove(state: State<AppState>, id: String, delete_file: bool, confirm: bool) -> Result<Vec<ModelRow>, String> {
    let (_, _, path) = entry_path(&state, &id)?;
    if delete_file {
        refuse_if_in_use(&state, &path)?;
        let plan = catalog_removal_plan(state.clone(), id.clone())?;
        if !plan.warnings.is_empty() && !confirm {
            return Err(plan.warnings.join("; "));
        }
        if path.is_file() {
            fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        }
    }
    {
        let _g = state.catalog_lock.lock().unwrap_or_else(|e| e.into_inner());
        let root = data_root(&state)?;
        let mut cat = catalog::load(&root)?;
        cat.models.retain(|e| e.id != id);
        catalog::save(&root, &cat)?;
    }
    Ok(scan_catalog(&state)?.1)
}

// --- Build ---

#[derive(Serialize)]
pub struct BuildsView {
    /// Build installate su questa macchina.
    pub installed: Vec<ResolvedBuild>,
    /// Quante profili usano ciascuna build, per id.
    pub used_by: BTreeMap<String, u32>,
    pub releases: Vec<Release>,
    pub releases_error: Option<String>,
}

#[tauri::command]
pub fn builds_list(state: State<AppState>, refresh: bool) -> Result<BuildsView, String> {
    let (root, machine) = root_and_machine(&state)?;
    let mut used_by: BTreeMap<String, u32> = BTreeMap::new();
    for p in profiles_of(&root) {
        *used_by.entry(format!("{} · {}", p.runtime.build, p.runtime.backend)).or_default() += 1;
    }
    let (releases, releases_error) = if refresh {
        match builds::releases(10) {
            Ok(r) => (r, None),
            Err(e) => (Vec::new(), Some(e)),
        }
    } else {
        (Vec::new(), None)
    };
    Ok(BuildsView { installed: root.builds_available(&machine), used_by, releases, releases_error })
}

/// Scarica una release, verifica il digest e la installa in `builds/`.
#[tauri::command]
pub fn builds_install(app: AppHandle, state: State<AppState>, tag: String, backend: String) -> Result<String, String> {
    let root = data_root(&state)?;
    let releases = builds::releases(10)?;
    let release = releases.iter().find(|r| r.tag == tag).ok_or_else(|| format!("release «{tag}» non trovata"))?;
    let asset = release
        .assets
        .iter()
        .find(|a| a.backend == backend)
        .cloned()
        .ok_or_else(|| format!("«{tag}» non ha un pacchetto per {backend}"))?;
    let target = builds::dir_name(&tag, &backend);
    let (task_id, cancel) = state.tasks.start(Kind::Install, &target)?;
    let tid = task_id.clone();
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let tasks = state.tasks.clone();
        let progress_id = tid.clone();
        let free = system::probe().free_disk_bytes(&root.builds());
        let result = builds::install(&root, &tag, &asset, free, &cancel, &mut |done, total| {
            tasks.progress(&progress_id, done, total)
        })
        .map(|i| format!("installata {} · {}", i.id, i.version_text.unwrap_or_default().lines().next().unwrap_or("").trim()));
        state.tasks.finish(&tid, result);
    });
    Ok(task_id)
}

/// `--list-devices` di una build installata: che cosa vede quel backend su questa macchina.
#[tauri::command]
pub fn build_devices(state: State<AppState>, id: String) -> Result<Vec<crate::memory::Device>, String> {
    let (root, machine) = root_and_machine(&state)?;
    let build = root
        .builds_available(&machine)
        .into_iter()
        .find(|b| b.id == id)
        .ok_or_else(|| format!("nessuna build «{id}» installata"))?;
    crate::memory::list_devices(&build.binary)
}

/// Registra nel catalogo un `.gguf` che sta fuori dalla cartella dei pesi. Non tocca niente:
/// dice che cosa comporterebbe farlo.
#[tauri::command]
pub fn catalog_import_plan(state: State<AppState>, path: String) -> Result<adopt::Plan, String> {
    let (_, machine) = root_and_machine(&state)?;
    let dir = machine.models_dir.ok_or("cartella dei pesi non impostata (Impostazioni → machine.toml)")?;
    adopt::plan(Path::new(path.trim()), &dir, system::probe().as_ref())
}

#[derive(Serialize)]
pub struct ImportOutcome {
    /// Il lavoro aperto, quando l'operazione dura (la copia). Il collegamento è immediato.
    pub task: Option<String>,
    pub message: String,
    pub rows: Vec<ModelRow>,
}

/// Esegue il piano. Un collegamento è immediato; una copia parte come lavoro con avanzamento e si
/// può fermare. Finita l'una o l'altra, il file è nella cartella e il catalogo lo trova da solo.
#[tauri::command]
pub fn catalog_import(app: AppHandle, state: State<AppState>, path: String, confirm_copy: bool) -> Result<ImportOutcome, String> {
    let plan = catalog_import_plan(state.clone(), path)?;
    if let Some(b) = &plan.blocker {
        return Err(b.clone());
    }
    match plan.action {
        adopt::Action::Nothing => {
            let rows = scan_catalog(&state)?.1;
            let file = plan.already.clone().unwrap_or_else(|| plan.file.clone());
            Ok(ImportOutcome { task: None, message: format!("«{file}» era già registrabile così com'è: catalogo riletto."), rows })
        }
        adopt::Action::Link => {
            adopt::apply(&plan, false, &AtomicBool::new(false), &mut |_, _| {})?;
            let rows = scan_catalog(&state)?.1;
            // Il riconoscimento per hash parte da solo: finché non finisce, il file è «presente».
            let id = rows.iter().find(|r| r.file.eq_ignore_ascii_case(&plan.file)).map(|r| r.id.clone());
            let hashing = matches!(id.map(|id| catalog_verify(app, state.clone(), id)), Some(Ok(_)));
            let note = if hashing {
                " Lo SHA-256 si sta calcolando: finché non finisce il file resta «presente», non «verificato»."
            } else {
                ""
            };
            Ok(ImportOutcome {
                task: None,
                message: format!(
                    "«{}» collegato nella cartella dei pesi: lo stesso contenuto con due nomi, nessun byte in più.{note}",
                    plan.file
                ),
                rows,
            })
        }
        adopt::Action::Copy => {
            if !confirm_copy {
                return Err(plan.notes.join("; "));
            }
            let (task_id, cancel) = state.tasks.start(Kind::Copy, &plan.file)?;
            let tid = task_id.clone();
            let file = plan.file.clone();
            std::thread::spawn(move || {
                let state = app.state::<AppState>();
                let tasks = state.tasks.clone();
                let pid = tid.clone();
                let result = adopt::apply(&plan, true, &cancel, &mut |done, total| tasks.progress(&pid, done, Some(total)))
                    .map(|o| match o {
                        adopt::Outcome::Copied { bytes, .. } => format!("copiati {:.2} GB", bytes as f64 / 1e9),
                        adopt::Outcome::Cancelled { .. } => "copia fermata: niente è rimasto nella cartella".into(),
                        other => format!("{other:?}"),
                    });
                state.tasks.finish(&tid, result);
            });
            Ok(ImportOutcome {
                task: Some(task_id),
                message: format!("«{file}» in copia da un altro volume: il file compare nella cartella solo a copia finita."),
                rows: Vec::new(),
            })
        }
    }
}

/// Gemello di «Aggiungi build…» delle Impostazioni, nella scheda Build del Catalogo: dichiara in
/// `machine.toml` una cartella di llama.cpp già scaricata a mano.
#[tauri::command]
pub fn builds_import_dir(state: State<AppState>, path: String) -> Result<BuildsView, String> {
    let (root, mut machine) = root_and_machine(&state)?;
    let dir = PathBuf::from(path.trim());
    if !dir.is_dir() {
        return Err(format!("{} non è una cartella", dir.display()));
    }
    let binary = dir.join(machine::server_binary_name());
    if !binary.is_file() {
        return Err(format!("in {} non c'è {}: non è una build di llama.cpp", dir.display(), machine::server_binary_name()));
    }
    if machine.builds.iter().any(|b| b.path == dir) {
        return Err(format!("{} è già dichiarata in machine.toml", dir.display()));
    }
    let name = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let id = name.strip_prefix("llama-").unwrap_or(&name).to_string();
    if machine.builds.iter().any(|b| b.id == id) {
        return Err(format!("c'è già una build «{id}»: rinomina la cartella o toglila dalle Impostazioni"));
    }
    machine.builds.push(BuildEntry { id, path: dir });
    root.save_machine(&machine)?;
    builds_list(state, false)
}

#[derive(Serialize)]
pub struct CardReport {
    pub notes: Vec<String>,
    pub rows: Vec<ModelRow>,
}

/// Legge la model card del publisher e ne ricava il campionamento consigliato, con fonte e data.
/// Non sovrascrive quello che c'è già: un valore letto oggi non cancella uno messo a mano.
#[tauri::command]
pub fn catalog_modelcard(state: State<AppState>, id: String, replace: bool) -> Result<CardReport, String> {
    let (_, entry, _) = entry_path(&state, &id)?;
    let repo = entry.repo.clone().ok_or_else(|| {
        format!("«{}» non dichiara un repository: senza publisher non c'è una model card da leggere", entry.id)
    })?;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let found = modelcard::from_repo(&repo, &today)?;
    let mut notes = found.notes.clone();
    if !found.sampling.is_empty() {
        let had = !entry.sampling_by_mode.is_empty();
        let sampling = found.sampling.clone();
        update_entry(&state, &id, move |e| {
            for (mode, s) in sampling {
                if replace || !e.sampling_by_mode.contains_key(&mode) {
                    e.sampling_by_mode.insert(mode, s);
                }
            }
        })?;
        if had && !replace {
            notes.push("le modalità già presenti nel catalogo sono state lasciate come stavano".into());
        }
    }
    Ok(CardReport { notes, rows: scan_catalog(&state)?.1 })
}

// --- Lavori in corso ---

#[tauri::command]
pub fn tasks_list(state: State<AppState>) -> Vec<TaskView> {
    state.tasks.list()
}

#[tauri::command]
pub fn task_cancel(state: State<AppState>, id: String) -> Result<(), String> {
    state.tasks.cancel(&id)
}

#[tauri::command]
pub fn tasks_clear(state: State<AppState>) -> usize {
    state.tasks.clear_finished()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_file_is_recognised_however_it_is_written() {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("aethera-cmd-{n}"));
        fs::create_dir_all(dir.join("pesi")).unwrap();
        let file = dir.join("pesi").join("M.gguf");
        fs::write(&file, b"x").unwrap();

        // Lo stesso file scritto passando da una cartella e tornando indietro: è sempre lui,
        // e il motore acceso dev'essere riconosciuto anche se il percorso è scritto diversamente.
        let giro = dir.join("pesi").join("..").join("pesi").join("M.gguf");
        assert!(same_file(&file, &giro));
        // Su Windows le maiuscole del percorso non fanno un file diverso.
        assert!(same_file(&file, &PathBuf::from(file.display().to_string().to_uppercase())));

        let altro = dir.join("pesi").join("N.gguf");
        fs::write(&altro, b"x").unwrap();
        assert!(!same_file(&file, &altro), "stesso contenuto non vuol dire stesso file");
        // Percorsi che non esistono si confrontano come sono scritti, senza esplodere.
        assert!(same_file(Path::new("mai-esistito.gguf"), Path::new("mai-esistito.gguf")));
        assert!(!same_file(Path::new("uno.gguf"), Path::new("due.gguf")));
        let _ = fs::remove_dir_all(&dir);
    }
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
