//! Storico degli avvii da `runs/`: manifest e telemetria di ogni avvio, per la pagina Benchmark.
//! Aethera non lancia banchi: ogni riga è un avvio con le sue condizioni accanto.

use crate::conditions::{Conditions, Driver};
use crate::manifest::{self, Manifest};
use crate::profile::{self, Override};
use crate::provenance;
use crate::telemetry::{self, Record, Reference, Summary};
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct RunRow {
    pub id: String,
    pub started: String,
    pub ended: Option<String>,
    pub running: bool,
    pub machine: String,
    pub profile: String,
    pub overrides: Vec<Override>,
    pub invalidates_cache: bool,
    pub build: String,
    pub commit: Option<String>,
    pub uptime_s: Option<u64>,
    pub load_ms: Option<u64>,
    pub ctx_declared: u32,
    pub ctx_served: Option<u32>,
    pub load_mode: String,
    pub speculative: String,
    pub summary: Summary,
    pub vram_dedicated_gib: Option<f64>,
    pub ram_available_after_gib: Option<f64>,
    pub double_copy: Option<bool>,
    pub margin_ok: Option<bool>,
    pub exit_code: Option<i32>,
    pub by_user: Option<bool>,
    pub left_running: Option<bool>,
    pub degraded: Vec<String>,
    pub conditions: Option<Conditions>,
    /// Riga corta per la tabella: `GPU …1004 · NPU …3930 · max · VGM 48`.
    pub conditions_short: Option<String>,
    /// Che cosa è cambiato rispetto all'avvio precedente sulla stessa macchina: da qui la mediana
    /// di riferimento riparte, e le righe prima non si confrontano con queste senza dirlo.
    pub conditions_changed: Vec<String>,
    pub reference: Option<Reference>,
}

fn seconds_between(a: &str, b: &str) -> Option<u64> {
    let a = chrono::DateTime::parse_from_rfc3339(a).ok()?;
    let b = chrono::DateTime::parse_from_rfc3339(b).ok()?;
    u64::try_from((b - a).num_seconds()).ok()
}

fn row(m: &Manifest, records: &[Record], current: Option<&str>) -> RunRow {
    let running = current == Some(m.run.id.as_str());
    let ended = m.exit.as_ref().map(|e| e.at.clone());
    let uptime_s = match (&ended, running) {
        (Some(end), _) => seconds_between(&m.run.started, end),
        (None, true) => seconds_between(&m.run.started, &chrono::Local::now().to_rfc3339()),
        (None, false) => None,
    };
    let after = m.memory.as_ref().and_then(|x| x.after_load.as_ref());
    // Gli avvii precedenti a M-14 non registrano la serie: allora non esistevano build compilate
    // qui, quindi senza provenienza la build è di ggml-org.
    let conditions = m.conditions.clone().map(|mut c| {
        if c.build_series.is_none() {
            c.build_series = Some(series_of_manifest(m));
        }
        c
    });
    let mut summary = telemetry::summarize(records, usize::MAX, Some(m.effective_profile.server.ubatch));
    summary.cache_series.clear();
    summary.decode_series.clear();
    summary.turns.clear();
    let sp = &m.effective_profile.speculative;
    RunRow {
        id: m.run.id.clone(),
        started: m.run.started.clone(),
        ended,
        running,
        machine: m.run.machine.clone(),
        profile: m.run.profile.clone(),
        overrides: m.overrides.clone(),
        invalidates_cache: m.run.invalidates_cache,
        build: build_label(m),
        commit: m.engine.commit.clone(),
        uptime_s,
        load_ms: m.server.load_ms,
        ctx_declared: m.server.ctx_declared,
        ctx_served: m.server.ctx_served,
        load_mode: m.effective_profile.server.load_mode.clone(),
        speculative: match sp.draft_n_max {
            Some(n) if sp.kind != "none" => format!("{} {n}", sp.kind),
            _ => sp.kind.clone(),
        },
        degraded: Vec::new(),
        conditions_short: conditions.as_ref().map(Conditions::short),
        conditions,
        conditions_changed: Vec::new(),
        reference: None,
        summary,
        vram_dedicated_gib: after.and_then(|a| a.vram_dedicated_gib),
        ram_available_after_gib: after.and_then(|a| a.ram_available_gib),
        double_copy: after.and_then(|a| a.double_copy),
        margin_ok: after.and_then(|a| a.margin_ok),
        exit_code: m.exit.as_ref().and_then(|e| e.code),
        by_user: m.exit.as_ref().map(|e| e.by_user),
        left_running: m.exit.as_ref().map(|e| e.left_running),
    }
}

/// La build come la chiede un profilo: `b10991+moro1` per una build del fork (la versione del
/// binario dice solo `b10991`), altrimenti quella letta da `--version` o, se manca, la dichiarata.
fn build_label(m: &Manifest) -> String {
    if let Some(p) = &m.engine.provenance {
        return p.label();
    }
    if m.engine.provenance_error.is_some() || crate::machine::parse_build_tag(&m.engine.build_declared).is_some_and(|t| t.series.is_some()) {
        return m.engine.build_declared.clone();
    }
    m.engine.build.clone().unwrap_or_else(|| m.engine.build_declared.clone())
}

fn series_of_manifest(m: &Manifest) -> String {
    match (&m.engine.provenance, &m.engine.provenance_error) {
        (Some(p), _) => p.series(),
        (None, Some(e)) => provenance::series_of(Some(&Err(e.clone()))),
        (None, None) => provenance::series_of(None),
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Mediana di riferimento per un avvio: la mediana dei decode degli avvii precedenti (`older`)
/// con la stessa macchina, lo stesso profilo, la stessa build e le stesse condizioni. Un avvio
/// senza condizioni registrate (prima di M-09) non entra: non si sa con che driver è stato fatto.
pub fn reference(older: &[RunRow], machine: &str, profile: &str, build: &str, conditions: &Conditions) -> Option<Reference> {
    let key = conditions.comparable();
    let medians: Vec<f64> = older
        .iter()
        .filter(|r| r.machine == machine && r.profile == profile && r.build == build)
        .filter(|r| r.conditions.as_ref().is_some_and(|c| c.comparable() == key))
        .filter_map(|r| r.summary.decode_median)
        .collect();
    Some(Reference { decode_median: telemetry::median(&medians)?, runs: medians.len() })
}

/// Che cosa è cambiato rispetto all'avvio più recente fra `older`, sulla stessa macchina e con le
/// condizioni registrate.
pub fn changed_since(older: &[RunRow], machine: &str, conditions: &Conditions) -> Vec<String> {
    older
        .iter()
        .filter(|r| r.machine == machine)
        .find_map(|r| r.conditions.as_ref())
        .map(|before| conditions.changes_since(before))
        .unwrap_or_default()
}

/// Avvii leggibili, dal più recente. Le cartelle senza manifest valido si saltano.
pub fn list(runs: &Path, current: Option<&str>) -> Vec<RunRow> {
    let Ok(entries) = fs::read_dir(runs) else { return Vec::new() };
    let mut read: Vec<(RunRow, Vec<Record>)> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter_map(|dir| {
            let m = manifest::read(&dir.join("manifest.toml")).ok()?;
            let records = telemetry::read(&dir);
            Some((row(&m, &records, current), records))
        })
        .collect();
    read.sort_by(|a, b| b.0.started.cmp(&a.0.started));
    let plain: Vec<RunRow> = read.iter().map(|(r, _)| r.clone()).collect();
    for (i, (r, records)) in read.iter_mut().enumerate() {
        let older = &plain[i + 1..];
        if let Some(c) = r.conditions.clone() {
            r.reference = reference(older, &r.machine, &r.profile, &r.build, &c);
            r.conditions_changed = changed_since(older, &r.machine, &c);
        }
        r.degraded = r.uptime_s.map(|u| telemetry::degraded(u, records, r.reference.as_ref())).unwrap_or_default();
    }
    read.into_iter().map(|(r, _)| r).collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct RunDetail {
    pub row: RunRow,
    pub manifest_text: String,
    pub records: Vec<Record>,
}

pub fn detail(runs: &Path, id: &str, current: Option<&str>) -> Result<RunDetail, String> {
    if !valid_id(id) {
        return Err(format!("id di avvio non valido: {id}"));
    }
    let dir = runs.join(id);
    let path = dir.join("manifest.toml");
    let manifest_text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let m: Manifest = toml::from_str(&manifest_text).map_err(|e| format!("{}: {e}", path.display()))?;
    let records = telemetry::read(&dir);
    // Riferimento e cambi di condizioni dipendono dagli altri avvii: li calcola la lista.
    let row = list(runs, current).into_iter().find(|r| r.id == m.run.id).unwrap_or_else(|| row(&m, &records, current));
    Ok(RunDetail { row, manifest_text, records })
}

#[derive(Debug, Clone, Serialize)]
pub struct Condition {
    pub label: String,
    pub a: Option<String>,
    pub b: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Comparison {
    pub a: RunRow,
    pub b: RunRow,
    /// Differenze fra i profili effettivi dei due avvii.
    pub profile_diff: Vec<Override>,
    /// Condizioni della misura: macchina, build, driver, tempo acceso, carico, memoria.
    pub conditions: Vec<Condition>,
    /// Condizioni diverse fra i due avvii che da sole spostano i numeri: il Δ non è del solo profilo.
    pub not_comparable: Vec<String>,
}

fn opt<T: ToString>(x: Option<T>) -> Option<String> {
    x.map(|v| v.to_string())
}

/// Perché due avvii non si confrontano come se differissero solo per il profilo. Si leggono le
/// condizioni delle righe, già completate con la serie di patch.
fn not_comparable(ra: &RunRow, rb: &RunRow) -> Vec<String> {
    let mut out = Vec::new();
    if ra.machine != rb.machine {
        out.push(format!("macchina {} · {}", ra.machine, rb.machine));
    }
    match (&ra.conditions, &rb.conditions) {
        (Some(ca), Some(cb)) => {
            let kb = cb.comparable();
            for (k, va) in ca.comparable() {
                if let Some(vb) = kb.get(k).filter(|vb| **vb != va) {
                    out.push(format!("{k} {va} · {vb}"));
                }
            }
        }
        _ => out.push("condizioni di uno dei due avvii sconosciute: è precedente a quando Aethera le registra".into()),
    }
    out
}

pub fn compare(runs: &Path, a: &str, b: &str, current: Option<&str>) -> Result<Comparison, String> {
    let (da, db) = (detail(runs, a, current)?, detail(runs, b, current)?);
    let ma: Manifest = toml::from_str(&da.manifest_text).map_err(|e| e.to_string())?;
    let mb: Manifest = toml::from_str(&db.manifest_text).map_err(|e| e.to_string())?;
    let before = |m: &Manifest| m.memory.as_ref().and_then(|x| x.before.clone()).unwrap_or_default();
    let (ba, bb) = (before(&ma), before(&mb));
    let c = |label: &str, a: Option<String>, b: Option<String>| Condition { label: label.into(), a, b };
    let (ca, cb) = (da.row.conditions.clone().unwrap_or_default(), db.row.conditions.clone().unwrap_or_default());
    let drivers = |ds: &[Driver]| {
        (!ds.is_empty()).then(|| ds.iter().map(|d| d.version.clone().unwrap_or_else(|| "?".into())).collect::<Vec<_>>().join(" + "))
    };
    let vgm = |x: &Conditions| x.gpus.iter().find_map(|g| g.dedicated_gib).map(|v| format!("{v} GiB"));
    let volume = |x: &Conditions| match (&x.weights_volume, &x.weights_disk) {
        (Some(v), Some(d)) => Some(format!("{v} · {d}")),
        (v, _) => v.clone(),
    };
    let not_comparable = not_comparable(&da.row, &db.row);
    let conditions = vec![
        c("Macchina", Some(ma.run.machine.clone()), Some(mb.run.machine.clone())),
        c("Build", Some(da.row.build.clone()), Some(db.row.build.clone())),
        c("Commit", ma.engine.commit.clone(), mb.engine.commit.clone()),
        c("Serie di patch", ca.build_series.clone(), cb.build_series.clone()),
        c("Driver GPU", drivers(&ca.gpus), drivers(&cb.gpus)),
        c("AMD Software", ca.adrenalin.clone(), cb.adrenalin.clone()),
        c("Driver NPU", drivers(&ca.npus), drivers(&cb.npus)),
        c("Alimentazione", ca.overlay_label(), cb.overlay_label()),
        c("VGM", vgm(&ca), vgm(&cb)),
        c("Volume dei pesi", volume(&ca), volume(&cb)),
        c("Pesi (SHA-256 dichiarato)", ma.model.sha256_declared.clone(), mb.model.sha256_declared.clone()),
        c("Acceso per (s)", opt(da.row.uptime_s), opt(db.row.uptime_s)),
        c("Richieste", Some(da.row.summary.requests.to_string()), Some(db.row.summary.requests.to_string())),
        c("Pronto in (ms)", opt(da.row.load_ms), opt(db.row.load_ms)),
        c("Contesto servito", opt(da.row.ctx_served), opt(db.row.ctx_served)),
        c("VRAM libera prima (MiB)", opt(ba.vram_free_mib), opt(bb.vram_free_mib)),
        c("RAM disponibile prima (GiB)", opt(ba.ram_available_gib), opt(bb.ram_available_gib)),
        c("Doppia copia dei pesi", opt(da.row.double_copy), opt(db.row.double_copy)),
        c("Invalida la cache", Some(da.row.invalidates_cache.to_string()), Some(db.row.invalidates_cache.to_string())),
        c("Codice di uscita", opt(da.row.exit_code), opt(db.row.exit_code)),
    ];
    Ok(Comparison {
        profile_diff: profile::diff(&ma.effective_profile, &mb.effective_profile),
        conditions,
        not_comparable,
        a: da.row,
        b: db.row,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_like_ids() {
        assert!(valid_id("r-20260915-233520"));
        assert!(!valid_id("..\\x"));
        assert!(!valid_id(""));
        assert!(detail(Path::new("."), "../etc", None).is_err());
    }

    #[test]
    fn uptime_from_rfc3339() {
        assert_eq!(seconds_between("2026-09-15T23:35:20+02:00", "2026-09-15T23:35:51+02:00"), Some(31));
        assert_eq!(seconds_between("x", "2026-09-15T23:35:51+02:00"), None);
    }
}
