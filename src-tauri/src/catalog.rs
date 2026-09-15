//! Catalogo dei modelli: che cosa c'è su questa macchina, con quale hash e quali metadati.
//!
//! Due sorgenti si incontrano qui: `catalog.toml` nella radice dati (le voci dichiarate, con
//! repo, hash atteso e campionamento della model card) e la cartella dei pesi di `machine.toml`
//! (i file che ci sono davvero). Un file senza voce diventa una voce scoperta; una voce senza
//! file è scaricabile. Il riconoscimento è per nome del file e, quando l'hash è stato calcolato,
//! per hash: lo stesso file può essere un hard link creato da un altro strumento.
//!
//! I metadati GGUF costano ~1 s a file: si leggono una volta e si tengono in `catalog.toml`
//! con dimensione e data del file come marcatore di validità. Se il file cambia, si rileggono.

use crate::estimate::{self, Estimate, MeasuredCompute};
use crate::gguf::{self, ModelInfo};
use crate::machine::{DataRoot, MachineConfig};
use crate::profile::{Profile, Sampling, Server};
use crate::system::SystemProbe;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const CATALOG_FILE: &str = "catalog.toml";
pub const CATALOG_SCHEMA: u32 = 1;
/// Estensione di un download a metà: il file non è utilizzabile finché non è verificato.
pub const PART_SUFFIX: &str = ".part";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Catalog {
    pub schema_version: u32,
    #[serde(default, rename = "model", skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<Entry>,
}

impl Default for Catalog {
    fn default() -> Self {
        Self { schema_version: CATALOG_SCHEMA, models: Vec::new() }
    }
}

/// Una voce dichiarata: quello che sappiamo del modello a prescindere dal disco.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// Nome corto del modello, per esempio `qwen3.6-35b-a3b`.
    pub id: String,
    /// Repository sul publisher, per esempio `bartowski/Qwen_Qwen3.6-35B-A3B-GGUF`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    pub file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quant: Option<String>,
    /// GB decimali come li scrive il publisher.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_gb: Option<f64>,
    /// Hash atteso: l'oid LFS di Hugging Face, o quello dichiarato in un profilo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// Hash calcolato davvero sul file, con quando è stato fatto.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256_verified: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<String>,
    /// Campionamento consigliato dalla model card, per modalità: dato del modello, non del server.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sampling_by_mode: BTreeMap<String, Sampling>,
    /// Metadati GGUF già letti, validi finché il file non cambia.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gguf: Option<CachedInfo>,
}

impl Entry {
    pub fn discovered(file: &str) -> Self {
        Entry {
            id: id_from_file(file),
            repo: None,
            file: file.to_string(),
            quant: None,
            size_gb: None,
            sha256: None,
            sha256_verified: None,
            verified_at: None,
            sampling_by_mode: BTreeMap::new(),
            gguf: None,
        }
    }
}

/// Metadati letti dal GGUF, con il marcatore del file da cui vengono.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CachedInfo {
    pub size_bytes: u64,
    /// Data di modifica del file al momento della lettura.
    pub modified: String,
    pub info: ModelInfo,
}

/// `Qwen_Qwen3.6-35B-A3B-Q4_K_M.gguf` → `qwen_qwen3.6-35b-a3b-q4_k_m`.
fn id_from_file(file: &str) -> String {
    file.trim_end_matches(".gguf").trim_end_matches(".GGUF").to_ascii_lowercase()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// Hash calcolato e uguale a quello atteso.
    Verified,
    /// Il file c'è, l'hash non è stato ricalcolato (minuti su 20 GB).
    Present,
    /// Hash calcolato e **diverso** da quello atteso: file sbagliato o corrotto.
    Mismatch,
    /// C'è solo un `.part`: download da riprendere.
    Downloading,
    /// Dichiarato con un repository, ma non presente sul disco.
    Downloadable,
    /// Dichiarato senza repository e non presente: non si sa dove prenderlo.
    Missing,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelRow {
    pub id: String,
    pub repo: Option<String>,
    pub file: String,
    pub quant: Option<String>,
    /// Dichiarata dal publisher, in GB decimali.
    pub size_gb: Option<f64>,
    /// Misurata sul disco.
    pub size_bytes: Option<u64>,
    pub path: Option<String>,
    pub state: State,
    pub sha256: Option<String>,
    pub sha256_verified: Option<String>,
    pub verified_at: Option<String>,
    pub info: Option<ModelInfo>,
    /// Stima al contesto del profilo che lo usa: assente se nessun profilo lo cita.
    pub estimate: Option<Estimate>,
    pub estimate_ctx: Option<u32>,
    pub estimate_profile: Option<String>,
    /// Quanti nomi puntano allo stesso file: più di uno significa hard link di un altro strumento.
    pub hard_links: Option<u32>,
    /// Byte già scaricati in un `.part`.
    pub part_bytes: Option<u64>,
    pub profiles: Vec<String>,
    pub sampling_by_mode: BTreeMap<String, Sampling>,
}

pub fn path(root: &DataRoot) -> PathBuf {
    root.path.join(CATALOG_FILE)
}

/// Legge `catalog.toml`; se non c'è, il catalogo è vuoto (non è un errore).
pub fn load(root: &DataRoot) -> Result<Catalog, String> {
    let file = path(root);
    if !file.exists() {
        return Ok(Catalog::default());
    }
    let text = fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let c: Catalog = toml::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
    if c.schema_version != CATALOG_SCHEMA {
        return Err(format!(
            "{}: schema_version {} non supportata (attesa {CATALOG_SCHEMA})",
            file.display(),
            c.schema_version
        ));
    }
    Ok(c)
}

pub fn save(root: &DataRoot, c: &Catalog) -> Result<(), String> {
    let file = path(root);
    let text = toml::to_string_pretty(c).map_err(|e| e.to_string())?;
    fs::write(&file, text).map_err(|e| format!("{}: {e}", file.display()))
}

fn modified_at(md: &fs::Metadata) -> String {
    md.modified()
        .map(|t| chrono::DateTime::<chrono::Local>::from(t).to_rfc3339_opts(chrono::SecondsFormat::Secs, false))
        .unwrap_or_default()
}

/// I file `.gguf` presenti nella cartella dei pesi, con quelli a metà (`.part`).
fn files_in(dir: &Path) -> (BTreeMap<String, fs::Metadata>, BTreeMap<String, u64>) {
    let (mut files, mut parts) = (BTreeMap::new(), BTreeMap::new());
    let Ok(entries) = fs::read_dir(dir) else { return (files, parts) };
    for e in entries.flatten() {
        let Ok(md) = e.metadata() else { continue };
        if !md.is_file() {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_string();
        if let Some(stem) = name.strip_suffix(PART_SUFFIX) {
            parts.insert(stem.to_string(), md.len());
        } else if name.to_ascii_lowercase().ends_with(".gguf") {
            files.insert(name, md);
        }
    }
    (files, parts)
}

/// Rilegge i metadati GGUF se mancano o se il file è cambiato. Ritorna `true` se ha letto.
fn refresh_info(entry: &mut Entry, path: &Path, md: &fs::Metadata) -> bool {
    let modified = modified_at(md);
    if entry.gguf.as_ref().is_some_and(|c| c.size_bytes == md.len() && c.modified == modified) {
        return false;
    }
    match gguf::read_info(path) {
        Ok(info) => {
            entry.gguf = Some(CachedInfo { size_bytes: md.len(), modified, info });
            true
        }
        // Un file non leggibile (download a metà rinominato, file diverso) non cancella quello che sapevamo.
        Err(_) => false,
    }
}

/// Stato di una voce dal confronto fra hash atteso, hash calcolato e presenza sul disco.
fn state_of(entry: &Entry, on_disk: bool, part: bool) -> State {
    if !on_disk {
        return match (part, &entry.repo) {
            (true, _) => State::Downloading,
            (false, Some(_)) => State::Downloadable,
            (false, None) => State::Missing,
        };
    }
    match (&entry.sha256, &entry.sha256_verified) {
        (Some(a), Some(b)) if a.eq_ignore_ascii_case(b) => State::Verified,
        (Some(_), Some(_)) => State::Mismatch,
        // Senza hash atteso, un hash calcolato è comunque una verifica di integrità del file.
        (None, Some(_)) => State::Verified,
        _ => State::Present,
    }
}

pub struct ScanInput<'a> {
    pub machine: &'a MachineConfig,
    pub profiles: &'a [Profile],
    /// Buffer di calcolo misurato per (ubatch, backend): chiave `<ubatch>|<backend>`.
    pub compute: &'a BTreeMap<String, MeasuredCompute>,
    pub probe: &'a dyn SystemProbe,
}

/// Unisce voci dichiarate e file presenti. Aggiorna il catalogo in memoria (metadati letti):
/// il chiamante decide se salvarlo.
pub fn scan(catalog: &mut Catalog, input: &ScanInput) -> Vec<ModelRow> {
    let dir = input.machine.models_dir.clone();
    let (files, parts) = dir.as_ref().map(|d| files_in(d)).unwrap_or_default();

    // I file senza voce diventano voci scoperte, così il catalogo dice tutto quello che c'è.
    for name in files.keys() {
        if !catalog.models.iter().any(|e| e.file.eq_ignore_ascii_case(name)) {
            catalog.models.push(Entry::discovered(name));
        }
    }

    let mut rows: Vec<ModelRow> = Vec::with_capacity(catalog.models.len());
    for entry in catalog.models.iter_mut() {
        let md = files.get(&entry.file).or_else(|| files.iter().find(|(k, _)| k.eq_ignore_ascii_case(&entry.file)).map(|(_, v)| v));
        let full = dir.as_ref().map(|d| d.join(&entry.file));
        if let (Some(md), Some(p)) = (md, full.as_ref()) {
            refresh_info(entry, p, md);
        }
        let info = entry.gguf.as_ref().map(|c| c.info.clone());

        // La stima vale al contesto di un profilo vero: senza profilo non si inventa un contesto.
        let using: Vec<&Profile> = input.profiles.iter().filter(|p| p.model.file.eq_ignore_ascii_case(&entry.file)).collect();
        let (mut est, mut est_ctx, mut est_profile) = (None, None, None);
        if let Some(p) = using.first() {
            let compute = compute_for(input.compute, &p.server, &p.runtime.backend);
            est = Some(estimate::estimate(info.as_ref(), &p.server, md.map(|m| m.len()), compute));
            est_ctx = Some(p.server.ctx);
            est_profile = Some(p.name.clone());
        }

        rows.push(ModelRow {
            state: state_of(entry, md.is_some(), parts.contains_key(&entry.file)),
            id: entry.id.clone(),
            repo: entry.repo.clone(),
            quant: entry.quant.clone(),
            size_gb: entry.size_gb,
            size_bytes: md.map(|m| m.len()),
            hard_links: full.as_ref().filter(|_| md.is_some()).and_then(|p| input.probe.hard_links(p)),
            path: full.map(|p| p.display().to_string()),
            sha256: entry.sha256.clone(),
            sha256_verified: entry.sha256_verified.clone(),
            verified_at: entry.verified_at.clone(),
            info,
            estimate: est,
            estimate_ctx: est_ctx,
            estimate_profile: est_profile,
            part_bytes: parts.get(&entry.file).copied(),
            profiles: using.iter().map(|p| p.name.clone()).collect(),
            sampling_by_mode: entry.sampling_by_mode.clone(),
            file: entry.file.clone(),
        });
    }
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    rows
}

/// Chiave di confrontabilità del buffer di calcolo: stesso ubatch e stesso backend.
pub fn compute_key(ubatch: u32, backend: &str) -> String {
    format!("{ubatch}|{backend}")
}

const GIB: f64 = (1u64 << 30) as f64;

/// Ricava il buffer di calcolo dagli avvii già misurati, invece di stimarlo con una formula:
/// è la VRAM dedicata misurata meno i pesi, la cache KV e lo stato di quell'avvio.
///
/// `info_for` dà i metadati GGUF del file di quell'avvio: senza, la KV non è calcolabile e
/// l'avvio si salta. Per ogni coppia (ubatch, backend) vince l'avvio più recente.
pub fn measured_compute(
    runs_dir: &Path,
    info_for: &dyn Fn(&str) -> Option<ModelInfo>,
) -> BTreeMap<String, MeasuredCompute> {
    let mut best: BTreeMap<String, (String, u64)> = BTreeMap::new();
    let Ok(entries) = fs::read_dir(runs_dir) else { return BTreeMap::new() };
    let mut dirs: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    for dir in dirs {
        let Ok(m) = crate::manifest::read(&dir.join("manifest.toml")) else { continue };
        let Some(dedicated) = m.memory.as_ref().and_then(|x| x.after_load.as_ref()).and_then(|a| a.vram_dedicated_gib)
        else {
            continue;
        };
        let info = info_for(&m.model.file);
        let p = &m.effective_profile;
        let e = estimate::estimate(info.as_ref(), &p.server, Some(m.model.size_bytes), None);
        // Senza la KV la differenza non direbbe niente: meglio nessun dato che un dato inventato.
        let (Some(kv), Some(weights)) = (e.kv_bytes, e.weights_bytes) else { continue };
        let known = weights + kv + e.state_bytes.unwrap_or(0);
        let measured = (dedicated * GIB) as u64;
        let Some(buffer) = measured.checked_sub(known).filter(|b| *b > 0) else { continue };
        best.insert(compute_key(p.server.ubatch, &p.runtime.backend), (m.run.id.clone(), buffer));
    }
    best.into_iter().map(|(k, (run_id, bytes))| (k, MeasuredCompute { run_id, bytes })).collect()
}

fn compute_for(map: &BTreeMap<String, MeasuredCompute>, s: &Server, backend: &str) -> Option<MeasuredCompute> {
    map.get(&compute_key(s.ubatch, backend))
        .map(|m| MeasuredCompute { run_id: m.run_id.clone(), bytes: m.bytes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::MachineConfig;
    use crate::system::UnknownProbe;

    struct Temp(PathBuf);

    impl Temp {
        fn new(tag: &str) -> Self {
            let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let p = std::env::temp_dir().join(format!("aethera-{tag}-{n}"));
            fs::create_dir_all(&p).unwrap();
            Temp(p)
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn profile(name: &str, file: &str, ctx: u32) -> Profile {
        let text = format!(
            r#"
schema_version = 1
name = "{name}"
[model]
file = "{file}"
[runtime]
kind = "llama.cpp"
backend = "vulkan"
build = "b10809"
[server]
host = "127.0.0.1"
port = 8081
ctx = {ctx}
n_parallel = 1
n_gpu_layers = 999
flash_attn = "on"
cache_type_k = "f16"
cache_type_v = "f16"
ubatch = 2048
batch = 4096
"#
        );
        crate::profile::load(&text, name).0.expect("profilo di prova valido")
    }

    fn machine(dir: &Path) -> MachineConfig {
        MachineConfig {
            schema_version: 1,
            name: "prova".into(),
            models_dir: Some(dir.to_path_buf()),
            ram_margin_gib: 16.0,
            builds: Vec::new(),
        }
    }

    fn scan_with(catalog: &mut Catalog, dir: &Path, profiles: &[Profile]) -> Vec<ModelRow> {
        let compute = BTreeMap::new();
        let m = machine(dir);
        scan(catalog, &ScanInput { machine: &m, profiles, compute: &compute, probe: &UnknownProbe })
    }

    #[test]
    fn files_without_an_entry_are_discovered() {
        let t = Temp::new("scoperta");
        fs::write(t.0.join("Modello-Q4_K_M.gguf"), b"non un gguf vero").unwrap();
        let mut c = Catalog::default();
        let rows = scan_with(&mut c, &t.0, &[]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "modello-q4_k_m");
        assert_eq!(rows[0].state, State::Present);
        assert_eq!(rows[0].size_bytes, Some(16));
        // Il file non è un GGUF leggibile: i metadati restano sconosciuti, non inventati.
        assert!(rows[0].info.is_none());
        // E la voce scoperta entra nel catalogo, che il chiamante può salvare.
        assert_eq!(c.models.len(), 1);
    }

    #[test]
    fn a_declared_entry_without_the_file_is_downloadable_or_missing() {
        let t = Temp::new("mancante");
        let mut c = Catalog::default();
        c.models.push(Entry { repo: Some("bartowski/X-GGUF".into()), ..Entry::discovered("X-Q4.gguf") });
        c.models.push(Entry::discovered("Y-Q4.gguf"));
        let rows = scan_with(&mut c, &t.0, &[]);
        assert_eq!(rows[0].state, State::Downloadable, "con repo si sa dove prenderlo");
        assert_eq!(rows[1].state, State::Missing, "senza repo no");
        assert_eq!(rows[0].size_bytes, None);
    }

    #[test]
    fn a_part_file_means_the_download_is_resumable() {
        let t = Temp::new("part");
        fs::write(t.0.join("Z-Q4.gguf.part"), vec![0u8; 1024]).unwrap();
        let mut c = Catalog::default();
        c.models.push(Entry { repo: Some("unsloth/Z".into()), ..Entry::discovered("Z-Q4.gguf") });
        let rows = scan_with(&mut c, &t.0, &[]);
        assert_eq!(rows[0].state, State::Downloading);
        assert_eq!(rows[0].part_bytes, Some(1024));
        // Un .part non è un modello usabile: non compare come file presente.
        assert_eq!(rows[0].size_bytes, None);
    }

    #[test]
    fn hashes_decide_verified_or_mismatch() {
        let t = Temp::new("hash");
        fs::write(t.0.join("A.gguf"), b"x").unwrap();
        fs::write(t.0.join("B.gguf"), b"x").unwrap();
        let mut c = Catalog::default();
        c.models.push(Entry {
            sha256: Some("ABC".into()),
            sha256_verified: Some("abc".into()),
            ..Entry::discovered("A.gguf")
        });
        c.models.push(Entry {
            sha256: Some("abc".into()),
            sha256_verified: Some("999".into()),
            ..Entry::discovered("B.gguf")
        });
        let rows = scan_with(&mut c, &t.0, &[]);
        assert_eq!(rows[0].state, State::Verified, "confronto senza distinzione di maiuscole");
        assert_eq!(rows[1].state, State::Mismatch);
    }

    #[test]
    fn the_estimate_follows_the_profile_that_uses_the_model() {
        let t = Temp::new("stima");
        fs::write(t.0.join("M.gguf"), vec![0u8; 2048]).unwrap();
        let mut c = Catalog::default();
        let rows = scan_with(&mut c, &t.0, &[profile("p1", "M.gguf", 65_536)]);
        assert_eq!(rows[0].profiles, vec!["p1".to_string()]);
        assert_eq!(rows[0].estimate_ctx, Some(65_536));
        assert_eq!(rows[0].estimate_profile.as_deref(), Some("p1"));
        // Senza metadati GGUF la stima conosce solo i pesi ed è dichiarata minima.
        let e = rows[0].estimate.as_ref().unwrap();
        assert_eq!(e.weights_bytes, Some(2048));
        assert!(e.total_is_lower_bound);
    }

    #[test]
    fn a_model_no_profile_uses_has_no_estimate() {
        let t = Temp::new("senza-profilo");
        fs::write(t.0.join("M.gguf"), vec![0u8; 8]).unwrap();
        let mut c = Catalog::default();
        let rows = scan_with(&mut c, &t.0, &[profile("p1", "Altro.gguf", 32_768)]);
        // Nessun profilo lo cita: un contesto di riferimento sarebbe inventato.
        assert!(rows[0].estimate.is_none());
        assert!(rows[0].profiles.is_empty());
    }

    #[test]
    fn catalog_survives_a_round_trip_on_disk() {
        let t = Temp::new("toml");
        let root = DataRoot::new(&t.0);
        let mut c = Catalog::default();
        c.models.push(Entry {
            repo: Some("bartowski/Qwen_Qwen3.6-35B-A3B-GGUF".into()),
            quant: Some("Q4_K_M".into()),
            size_gb: Some(22.29),
            sha256: Some("b46fedd3".into()),
            ..Entry::discovered("Qwen_Qwen3.6-35B-A3B-Q4_K_M.gguf")
        });
        save(&root, &c).unwrap();
        assert_eq!(load(&root).unwrap(), c);
        // Un catalogo che non esiste ancora non è un errore.
        assert_eq!(load(&DataRoot::new(t.0.join("vuota"))).unwrap(), Catalog::default());
    }
}
