//! Comandi chiamati dalla finestra (e dalla tray).

use crate::adopt;
use crate::builds::{self, Release};
use crate::catalog::{self, Catalog, ModelRow, ScanInput};
use crate::clients::{self, ClientSnippets, RunFacts};
use crate::diagnose::{self, Reason};
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
use crate::proposals;
use crate::provenance::{self, BuildProvenance};
use crate::runs::{self, Comparison, RunDetail, RunRow};
use crate::settings::ExitBehavior;
use crate::conditions::Conditions;
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
    /// Perché la radice dati scelta non è utilizzabile: disco staccato, cartella sparita, sola
    /// lettura. Finché è piena, Aethera non può fare niente e lo dice invece di fallire un gesto
    /// alla volta.
    pub data_root_error: Option<String>,
    pub machine: Option<MachineConfig>,
    pub machine_error: Option<String>,
    pub system: SystemReport,
    /// Driver, alimentazione e disco dei pesi come li vede adesso Windows.
    pub conditions: Conditions,
    pub exit_behavior: ExitBehavior,
    /// Il nome del profilo principale, così com'è salvato: non è detto che esista ancora fra i
    /// profili letti da `list_profiles`, spetta a chi legge accorgersene e dirlo.
    pub default_profile: Option<String>,
    pub builds: Vec<ResolvedBuild>,
    pub builds_missing: Vec<BuildEntry>,
    /// Provenienza di ogni build di `builds`, nello stesso ordine (M-14): tag, serie e rami del
    /// fork, oppure niente per una build scaricata.
    pub builds_provenance: Vec<BuildProvenance>,
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
        data_root_error: None,
        machine: None,
        machine_error: None,
        system: system::probe().report(),
        conditions: system::probe().conditions(None),
        exit_behavior: settings.exit_behavior,
        default_profile: settings.default_profile.clone(),
        builds: Vec::new(),
        builds_missing: Vec::new(),
        builds_provenance: Vec::new(),
        endpoint: state.endpoint.as_ref().ok().cloned(),
        endpoint_error: state.endpoint.as_ref().err().cloned(),
    };
    if let Some(path) = settings.data_root {
        let root = DataRoot::new(path);
        // Prima la cartella, poi il contenuto: un disco staccato e un machine.toml sbagliato sono
        // due guasti diversi e si riparano in due modi diversi.
        if let Err(e) = fs::create_dir_all(&root.path)
            .map_err(|e| diagnose::write_error("creazione", &root.path, &e))
            .and_then(|_| root.check_writable())
        {
            o.data_root_error = Some(e);
            return o;
        }
        match root.ensure(&o.system) {
            Ok(m) => {
                o.conditions = system::probe().conditions(m.models_dir.as_deref());
                o.builds = root.builds_available(&m);
                o.builds_provenance = builds_provenance(&o.builds);
                o.builds_missing =
                    m.builds.iter().filter(|b| !b.path.join(machine::server_binary_name()).is_file()).cloned().collect();
                o.machine = Some(m);
            }
            Err(e) => o.machine_error = Some(e),
        }
    }
    o
}

/// La provenienza di ogni build trovata, una per una e nello stesso ordine.
fn builds_provenance(builds: &[ResolvedBuild]) -> Vec<BuildProvenance> {
    builds.iter().map(|b| provenance::describe(&b.dir)).collect()
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
    let root = DataRoot::new(&path);
    root.ensure(&system::probe().report())?;
    root.check_writable()?;
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

#[derive(Serialize)]
pub struct MachineText {
    pub path: String,
    /// Il testo com'è sul disco: quando non si lascia leggere, si mostra invece di sparire.
    pub text: Option<String>,
    pub error: Option<String>,
}

/// `machine.toml` così com'è. Serve quando non è valido: senza vederlo non lo si corregge.
#[tauri::command]
pub fn machine_text(state: State<AppState>) -> Result<MachineText, String> {
    let root = data_root(&state)?;
    let path = root.machine_file();
    let (text, error) = match fs::read_to_string(&path) {
        Ok(t) => (Some(t), root.load_machine().err()),
        Err(e) => (None, Some(diagnose::write_error("lettura", &path, &e))),
    };
    Ok(MachineText { path: path.display().to_string(), text, error })
}

/// Rimette un `machine.toml` nuovo quando quello che c'è non si lascia leggere. Il vecchio non si
/// perde: viene messo da parte con la data, perché dentro poteva esserci la cartella dei pesi e
/// l'elenco delle build, e ritrovarli è più facile che riscriverli.
#[tauri::command]
pub fn machine_reset(state: State<AppState>) -> Result<Overview, String> {
    let root = data_root(&state)?;
    let path = root.machine_file();
    if path.is_file() {
        let backup = root.path.join(format!("machine.toml.{}.bak", chrono::Local::now().format("%Y%m%d-%H%M%S")));
        fs::rename(&path, &backup).map_err(|e| diagnose::write_error("messa da parte", &backup, &e))?;
    }
    root.ensure(&system::probe().report())?;
    Ok(build_overview(&state))
}

#[tauri::command]
pub fn set_exit_behavior(state: State<AppState>, behavior: ExitBehavior) -> Result<(), String> {
    let mut s = state.settings();
    s.exit_behavior = behavior;
    s.save()
}

/// Il profilo principale così com'è salvato in `settings.toml`, senza controllare se esiste
/// ancora: `overview` lo porta già, questo comando serve a chi vuole rileggerlo da solo.
#[tauri::command]
pub fn default_profile(state: State<AppState>) -> Option<String> {
    state.settings().default_profile.clone()
}

/// Fissa (o toglie, con `None`) il profilo principale. Il nome non si controlla qui: un profilo
/// rinominato o cancellato dopo non cancella l'impostazione da solo, la dice sbagliata chi la
/// legge (la pagina Avvio, la tray).
#[tauri::command]
pub fn set_default_profile(state: State<AppState>, name: Option<String>) -> Result<Overview, String> {
    let name = name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
    {
        let mut s = state.settings();
        s.default_profile = name;
        s.save()?;
    }
    Ok(build_overview(&state))
}

/// Un passo della prima configurazione: che cos'è, se è fatto, e dove si fa.
#[derive(Serialize)]
pub struct Step {
    pub id: &'static str,
    pub title: &'static str,
    /// Che cosa fare, in una frase. È la stessa frase che compare negli stati vuoti della pagina.
    pub what: &'static str,
    pub done: bool,
    /// Che cosa c'è già, quando c'è: il passo fatto si legge, non si ricontrolla.
    pub detail: Option<String>,
    pub page: &'static str,
}

#[derive(Serialize)]
pub struct Setup {
    pub steps: Vec<Step>,
    pub complete: bool,
}

fn step(id: &'static str, title: &'static str, what: &'static str, page: &'static str, detail: Option<String>) -> Step {
    Step { id, title, what, done: detail.is_some(), detail, page }
}

/// Dove sta la prima configurazione: dalla radice dati al primo avvio, nell'ordine in cui i passi
/// dipendono l'uno dall'altro. Un passo è fatto quando c'è la cosa che deve esserci, non quando
/// qualcuno l'ha spuntato: così la guida dice sempre la verità anche se si arriva da metà strada.
#[tauri::command]
pub async fn setup(state: State<'_, AppState>) -> Result<Setup, String> {
    let o = build_overview(&state);
    let root_ok = o.data_root.is_some() && o.data_root_error.is_none() && o.machine.is_some();
    let mut steps = vec![step(
        "radice",
        "Radice dati",
        "Scegli la cartella dove Aethera tiene machine.toml, i profili, le build e gli avvii.",
        "impostazioni",
        root_ok.then(|| o.data_root.clone().unwrap_or_default()),
    )];

    let root = data_root(&state).ok();
    let machine = o.machine.clone();
    steps.push(step(
        "pesi",
        "Cartella dei pesi",
        "Dì dove tieni i .gguf: i profili li citano per nome file, la cartella è della macchina.",
        "impostazioni",
        machine
            .as_ref()
            .and_then(|m| m.models_dir.clone())
            .filter(|d| d.is_dir())
            .map(|d| d.display().to_string()),
    ));
    steps.push(step(
        "build",
        "Una build di llama.cpp",
        "Scaricane una dal Catalogo, o dichiara una cartella che hai già.",
        "catalogo",
        (!o.builds.is_empty()).then(|| {
            o.builds.iter().map(|b| b.id.clone()).collect::<Vec<_>>().join(" · ")
        }),
    ));

    // Basta sapere quali .gguf ci sono: niente metadati. La scansione completa (con la lettura dei
    // GGUF e la cache nel catalogo) resta al Catalogo; qui una lettura per file costerebbe 1-2 s e
    // questa funzione la finestra la chiama ogni pochi secondi.
    let models: Vec<String> = machine
        .as_ref()
        .and_then(|m| m.models_dir.as_ref())
        .and_then(|d| std::fs::read_dir(d).ok())
        .map(|entries| {
            let mut v: Vec<String> = entries
                .flatten()
                .filter(|e| e.file_type().map(|f| f.is_file()).unwrap_or(false))
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| n.to_ascii_lowercase().ends_with(".gguf"))
                .collect();
            v.sort();
            v
        })
        .unwrap_or_default();
    steps.push(step(
        "modello",
        "Un modello",
        "Aggiungi i pesi da Hugging Face e scaricali, oppure registra un .gguf che hai già sul disco.",
        "catalogo",
        (!models.is_empty()).then(|| format!("{} sul disco: {}", models.len(), models.join(" · "))),
    ));

    let profiles: Vec<String> = root
        .as_ref()
        .and_then(|r| read_profiles(r).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.profile.is_some())
        .map(|e| e.name)
        .collect();
    steps.push(step(
        "profilo",
        "Un profilo",
        "Dalla pagina Avvio, «Nuovo profilo…»: nasce con valori sensati sul modello che scegli.",
        "avvio",
        (!profiles.is_empty()).then(|| profiles.join(" · ")),
    ));

    let runs = root.as_ref().map(|r| runs::list(&r.runs(), None).len()).unwrap_or(0);
    steps.push(step(
        "avvio",
        "Il primo avvio",
        "Scegli il profilo e premi Avvia: da qui in poi la pagina Motore misura quello che succede.",
        "avvio",
        (runs > 0 || state.engine.is_running())
            .then(|| if runs > 0 { format!("{runs} avvii registrati") } else { "motore acceso".into() }),
    ));

    let complete = steps.iter().all(|s| s.done);
    Ok(Setup { steps, complete })
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
        .filter_map(|path| {
            let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let (profile, issues) = match fs::read_to_string(&path) {
                // Un profilo di servizio (M-20) sta nella stessa cartella ma non è un profilo di
                // avvio: lo legge `services::list_profiles`. Senza questo salto comparirebbe qui
                // come illeggibile, perché `role` e `[service]` non appartengono a un profilo
                // principale — ed è giusto che non vi appartengano.
                Ok(text) if crate::service::is_service(&text) => return None,
                Ok(text) => profile::load(&text, &stem),
                Err(e) => (None, vec![Issue { field: String::new(), message: e.to_string() }]),
            };
            Some(ProfileEntry { name: stem, file: path.display().to_string(), profile, issues })
        })
        .collect())
}

#[tauri::command]
pub async fn list_profiles(state: State<'_, AppState>) -> Result<Vec<ProfileEntry>, String> {
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
    /// Modifiche che le misure di M-08 suggeriscono per questo profilo: si applicano a mano.
    pub proposals: Vec<proposals::Proposal>,
    /// Avvisi che non impediscono l'avvio.
    pub warnings: Vec<String>,
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
pub async fn preview(state: State<'_, AppState>, base: String, edited: Profile) -> Result<Preview, String> {
    let (root, m) = root_and_machine(&state)?;
    let p = launch::prepare(&root, &m, &base, &edited, &root.runs().join("{run}").join("slots"));
    let binary = p.build.as_ref().map(|b| b.binary.clone()).unwrap_or_else(|| PathBuf::from(machine::server_binary_name()));
    let vgm = system::probe().report().gpus.iter().filter_map(|g| g.dedicated_gib).reduce(f64::max);
    Ok(Preview {
        proposals: proposals::for_profile(&edited, &root.builds_available(&m)),
        warnings: proposals::warnings(&edited, p.model_size, vgm),
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
    let mut conditions = system::probe().conditions(m.models_dir.as_deref());
    // M-14: la serie di patch è una condizione; va messa prima di cercare il riferimento, così la
    // mediana di una build patchata non si mescola con quella della build liscia.
    if let Some(b) = &p.build {
        conditions.build_series = Some(crate::provenance::series_of(crate::provenance::read(&b.dir).as_ref()));
    }
    let history = runs::list(&root.runs(), None);
    let reference = runs::reference(&history, &m.name, &edited.name, &edited.runtime.build, &conditions);
    let conditions_changed = runs::changed_since(&history, &m.name, &conditions);
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
        conditions,
        reference,
        conditions_changed,
    })
}

#[tauri::command]
pub fn engine_start(state: State<AppState>, base: String, edited: Profile) -> Result<RunInfo, String> {
    start_from(&state, &base, edited)
}

/// «Avvia il profilo principale» dalla tray: stessi controlli dell'avvio dalla pagina Avvio, letti
/// da `start_from` (motore già acceso, porta occupata, build non risolta, blocchi del profilo).
/// Se il nome salvato non è più un profilo valido non si inventa niente: lo dice.
pub fn start_default(state: &AppState) -> Result<RunInfo, String> {
    let name = state.settings().default_profile.clone().ok_or("nessun profilo principale scelto: fissalo nelle Impostazioni")?;
    let entry = read_profiles(&data_root(state)?)?
        .into_iter()
        .find(|e| e.name == name)
        .ok_or_else(|| format!("il profilo principale «{name}» non esiste più: scegline un altro nelle Impostazioni"))?;
    let profile = entry.profile.ok_or_else(|| {
        let why = entry.issues.iter().map(|i| i.message.clone()).collect::<Vec<_>>().join("; ");
        format!("il profilo principale «{name}» non si legge: {why}")
    })?;
    start_from(state, &name, profile)
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

/// Le righe del log che spiegano l'uscita con errore, invece del solo codice di uscita.
#[tauri::command]
pub fn engine_failure(state: State<AppState>) -> Vec<Reason> {
    let Some(path) = state.engine.log_path() else { return Vec::new() };
    let Ok(text) = engine::read_tail(&path, 400) else { return Vec::new() };
    diagnose::engine_failure(&text, 6)
}

#[tauri::command]
pub fn orphan_terminate(state: State<AppState>, pid: u32) -> Result<(), String> {
    state.engine.terminate_orphan(pid)
}

pub fn snippets_for(state: &AppState) -> Option<ClientSnippets> {
    let m = state.engine.manifest()?;
    let base_url = format!("http://{}:{}", m.server.host, m.server.port);
    // Prompt fissi: quelli misurati da M-08, più la prima richiesta a freddo di ogni client che in
    // questo avvio ha preso il lock (solo così il log sa chi è).
    let mut fixed = clients::measured_fixed_prompts();
    if let Some((_, summary, _)) = state.engine.recent(usize::MAX) {
        for f in clients::fixed_from_turns(&summary.turns, &m.run.id) {
            fixed.retain(|x| x.client != f.client);
            fixed.push(f);
        }
    }
    let claude_config_dir = data_root(state).ok().map(|r| r.path.join("clients").join("claude-code").display().to_string());
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
        chat_template: m.effective_profile.server.chat_template_file.as_deref(),
        claude_config_dir,
        fixed_prompts: &fixed,
        services: &state.services.view(system::probe().as_ref()),
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
pub async fn runs_list(state: State<'_, AppState>) -> Result<Vec<RunRow>, String> {
    Ok(runs::list(&data_root(&state)?.runs(), current_run(&state).as_deref()))
}

#[tauri::command]
pub async fn run_detail(state: State<'_, AppState>, id: String) -> Result<RunDetail, String> {
    runs::detail(&data_root(&state)?.runs(), &id, current_run(&state).as_deref())
}

#[tauri::command]
pub async fn runs_compare(state: State<'_, AppState>, a: String, b: String) -> Result<Comparison, String> {
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
fn scan_catalog(state: &AppState) -> Result<(Catalog, Vec<ModelRow>, Option<String>), String> {
    let (root, machine) = root_and_machine(state)?;
    let profiles = profiles_of(&root);
    let probe = system::probe();
    // Un `catalog.toml` illeggibile non ferma la pagina e **non viene riscritto**: si lavora su un
    // catalogo vuoto, che vede comunque i file nella cartella dei pesi, e si dice perché.
    let (mut cat, error) = match catalog::load(&root) {
        Ok(c) => (c, None),
        Err(e) => (
            Catalog::default(),
            Some(format!(
                "{e} — Aethera non lo sovrascrive: correggilo o rinominalo. Intanto il Catalogo mostra solo i file che ci sono nella cartella dei pesi, e hash e campionamento già registrati restano dentro quel file."
            )),
        ),
    };
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

    if error.is_none() && cat != before {
        catalog::save(&root, &cat)?;
    }
    Ok((cat, rows, error))
}

#[derive(Serialize)]
pub struct CatalogView {
    pub rows: Vec<ModelRow>,
    /// Perché il catalogo dichiarato non è stato letto. Le righe restano valide: vengono dal disco.
    pub error: Option<String>,
}

#[tauri::command]
pub async fn catalog_list(state: State<'_, AppState>) -> Result<CatalogView, String> {
    let (_, rows, error) = scan_catalog(&state)?;
    Ok(CatalogView { rows, error })
}

/// Ricollega una voce a un file diverso: i pesi sono stati rinominati e il catalogo li ha persi.
/// L'hash calcolato in passato non vale più per un altro file: si butta, invece di portarselo
/// dietro come se fosse ancora vero.
#[tauri::command]
pub fn catalog_relink(state: State<AppState>, id: String, file: String) -> Result<Vec<ModelRow>, String> {
    let (_, machine) = root_and_machine(&state)?;
    let dir = machine.models_dir.ok_or("cartella dei pesi non impostata (Impostazioni → machine.toml)")?;
    let file = file.trim().to_string();
    if file.contains(['/', '\\']) {
        return Err("solo il nome del file: la cartella dei pesi è della macchina".into());
    }
    if !dir.join(&file).is_file() {
        return Err(format!("in {} non c'è nessun «{file}»", dir.display()));
    }
    {
        let _g = state.catalog_lock.lock().unwrap_or_else(|e| e.into_inner());
        let root = data_root(&state)?;
        let mut cat = catalog::load(&root)?;
        if cat.models.iter().any(|e| e.id != id && e.file.eq_ignore_ascii_case(&file)) {
            return Err(format!("«{file}» è già di un'altra voce del catalogo"));
        }
        let entry = cat.models.iter_mut().find(|e| e.id == id).ok_or_else(|| format!("nessuna voce «{id}»"))?;
        entry.file = file;
        entry.sha256_verified = None;
        entry.verified_at = None;
        entry.gguf = None;
        catalog::save(&root, &cat)?;
    }
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
pub async fn builds_list(state: State<'_, AppState>, refresh: bool) -> Result<BuildsView, String> {
    builds_view(&state, refresh)
}

/// Il corpo di `builds_list`, sincrono: lo usa anche `builds_import_dir`.
fn builds_view(state: &AppState, refresh: bool) -> Result<BuildsView, String> {
    let (root, machine) = root_and_machine(state)?;
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
    builds_view(&state, false)
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

    #[test]
    fn every_build_found_carries_its_provenance() {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("aethera-cmd-prov-{n}"));
        let (plain, fork) = (dir.join("llama-b10991-win-vulkan-x64"), dir.join("llama-b10991+moro1-win-vulkan-x64"));
        fs::create_dir_all(&plain).unwrap();
        fs::create_dir_all(&fork).unwrap();
        let file = [
            "schema_version = 1",
            "id = \"b10991+moro1-vulkan\"",
            "base = \"b10991\"",
            "commit_base = \"930e2fa5995789efbf249a8bf61325bb626e417b\"",
            "serie = 1",
            "backend = \"vulkan\"",
            "commit = \"0123456789abcdef0123456789abcdef01234567\"",
            "durata_build_s = 640",
            "",
            "[[patch]]",
            "ramo = \"patch/int8-coopmat\"",
            "commit = \"abcdef0123456789abcdef0123456789abcdef01\"",
        ]
        .join("\n");
        fs::write(fork.join(provenance::PROVENANCE_FILE), file).unwrap();
        let rb = |d: &Path| ResolvedBuild { id: "x".into(), dir: d.to_path_buf(), binary: d.join("llama-server.exe"), source: "builds/" };

        let out = builds_provenance(&[rb(&plain), rb(&fork)]);
        assert_eq!(out.len(), 2);
        // La build scaricata non ha provenienza e non ha errore; quella del fork porta la serie.
        assert_eq!(out[0].dir, plain);
        assert!(out[0].provenance.is_none() && out[0].error.is_none());
        let p = out[1].provenance.as_ref().expect("provenienza della build del fork");
        assert_eq!(p.label(), "b10991+moro1");
        assert_eq!(p.series(), "moro1 patch/int8-coopmat@abcdef012");
        assert_eq!(p.durata_build_s, Some(640));
        let _ = fs::remove_dir_all(&dir);
    }
}

/// Risposta al dialogo «Esci» quando il motore è acceso o c'è un lavoro in corso.
///
/// Un hash o un download a metà non si troncano in silenzio: o li si lascia finire, o si dice di
/// fermarli — e allora si **aspetta** che si fermino davvero, perché un `.part` è utile solo se
/// quello che c'è dentro è arrivato sul disco.
#[tauri::command]
pub fn app_exit(
    app: AppHandle,
    state: State<AppState>,
    stop: bool,
    remember: bool,
    cancel_tasks: Option<bool>,
) -> Result<(), String> {
    let busy = state.tasks.running();
    if !busy.is_empty() {
        if cancel_tasks != Some(true) {
            return Err(format!(
                "{} lavori sono in corso ({}): fermali o aspetta che finiscano",
                busy.len(),
                busy.iter().map(|t| t.target.clone()).collect::<Vec<_>>().join(", ")
            ));
        }
        state.tasks.cancel_all();
        // I lavori si accorgono della bandiera al prossimo blocco: si dà loro il tempo di chiudere
        // il file. Se non bastano dieci secondi si esce comunque, dicendolo nel messaggio.
        let deadline = Instant::now() + Duration::from_secs(10);
        while !state.tasks.running().is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
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

// ─────────────────────────────────────────────────────────────────────────────
// Motori di servizio (M-20)
//
// Sono volutamente pochi comandi e nessuna magia: elenca, accendi, spegni. Un servizio non ha
// «anteprima», «salva come» né «riavvia con la stessa riga», perché non è il soggetto di una
// misura e non c'è nessun avvio precedente da riprodurre.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct ServiceEntry {
    pub profile: crate::service::ServiceProfile,
    pub issues: Vec<Issue>,
    /// Lo stato ora, se è acceso.
    pub running: Option<crate::services::ServiceView>,
    /// Perché non si può accendere: pesi mancanti, build mancante, porta occupata.
    pub blockers: Vec<String>,
}

fn service_paths(state: &AppState, p: &crate::service::ServiceProfile) -> Result<(PathBuf, PathBuf), String> {
    let (_, machine) = root_and_machine(state)?;
    let dir = machine
        .models_dir
        .clone()
        .ok_or("cartella dei pesi non impostata (Impostazioni → machine.toml)")?;
    let root = data_root(state)?;
    let builds = root.builds_available(&machine);
    let build = machine::resolve_build(&builds, &p.runtime.build, &p.runtime.backend).ok_or_else(|| {
        format!("build «{} {}» non dichiarata su questa macchina", p.runtime.build, p.runtime.backend)
    })?;
    Ok((build.binary.clone(), crate::service::model_path(&dir, p)))
}

#[tauri::command]
pub fn services_list(state: State<AppState>) -> Result<Vec<ServiceEntry>, String> {
    let root = data_root(&state)?;
    let viste = state.services.view(system::probe().as_ref());
    Ok(crate::services::list_profiles(&root.profiles())
        .into_iter()
        .map(|(profile, issues)| {
            let running = viste.iter().find(|v| v.name == profile.name).cloned();
            let mut blockers = Vec::new();
            match service_paths(&state, &profile) {
                Ok((binary, model)) => {
                    if !binary.is_file() {
                        blockers.push(format!("manca il binario della build: {}", binary.display()));
                    }
                    if !model.is_file() {
                        blockers.push(format!("mancano i pesi: {}", model.display()));
                    }
                }
                Err(e) => blockers.push(e),
            }
            ServiceEntry { profile, issues, running, blockers }
        })
        .collect())
}

#[tauri::command]
pub async fn service_start(state: State<'_, AppState>, name: String) -> Result<crate::services::ServiceView, String> {
    let root = data_root(&state)?;
    let (profile, issues) = crate::services::list_profiles(&root.profiles())
        .into_iter()
        .find(|(p, _)| p.name == name)
        .ok_or_else(|| format!("nessun profilo di servizio «{name}»"))?;
    if !issues.is_empty() {
        return Err(format!("il profilo «{name}» ha errori: {}", issues[0].message));
    }
    let (binary, model) = service_paths(&state, &profile)?;
    let kind = profile.service.kind.clone();
    let view = state.services.start(crate::services::StartService { profile, binary, model })?;

    // Un `/health` verde non basta per il reranker: un GGUF convertito male risponde 200 e poi dà
    // punteggi vicini a zero anche al documento giusto. Il controllo si fa una volta, all'avvio.
    if kind == "rerank" {
        match crate::services::rerank_sanity(&view.base_url) {
            Ok(score) => state.services.set_sane(&name, score > 0.5),
            Err(_) => state.services.set_sane(&name, false),
        }
    }
    Ok(view)
}

#[tauri::command]
pub async fn service_stop(state: State<'_, AppState>, name: String) -> Result<(), String> {
    state.services.stop(&name)
}
