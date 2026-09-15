//! Storico degli avvii da `runs/`: manifest e telemetria di ogni avvio, per la pagina Benchmark.
//! Aethera non lancia banchi: ogni riga è un avvio con le sue condizioni accanto.

use crate::manifest::{self, Manifest};
use crate::profile::{self, Override};
use crate::telemetry::{self, Record, Summary};
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
    let mut summary = telemetry::summarize(records, usize::MAX);
    summary.cache_series.clear();
    summary.decode_series.clear();
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
        build: m.engine.build.clone().unwrap_or_else(|| m.engine.build_declared.clone()),
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
        degraded: uptime_s.map(|u| telemetry::degraded(u, records)).unwrap_or_default(),
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

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Avvii leggibili, dal più recente. Le cartelle senza manifest valido si saltano.
pub fn list(runs: &Path, current: Option<&str>) -> Vec<RunRow> {
    let Ok(entries) = fs::read_dir(runs) else { return Vec::new() };
    let mut rows: Vec<RunRow> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter_map(|dir| {
            let m = manifest::read(&dir.join("manifest.toml")).ok()?;
            Some(row(&m, &telemetry::read(&dir), current))
        })
        .collect();
    rows.sort_by(|a, b| b.started.cmp(&a.started));
    rows
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
    Ok(RunDetail { row: row(&m, &records, current), manifest_text, records })
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
    /// Condizioni della misura: macchina, build, tempo acceso, carico, memoria.
    pub conditions: Vec<Condition>,
}

fn opt<T: ToString>(x: Option<T>) -> Option<String> {
    x.map(|v| v.to_string())
}

pub fn compare(runs: &Path, a: &str, b: &str, current: Option<&str>) -> Result<Comparison, String> {
    let (da, db) = (detail(runs, a, current)?, detail(runs, b, current)?);
    let ma: Manifest = toml::from_str(&da.manifest_text).map_err(|e| e.to_string())?;
    let mb: Manifest = toml::from_str(&db.manifest_text).map_err(|e| e.to_string())?;
    let before = |m: &Manifest| m.memory.as_ref().and_then(|x| x.before.clone()).unwrap_or_default();
    let (ba, bb) = (before(&ma), before(&mb));
    let c = |label: &str, a: Option<String>, b: Option<String>| Condition { label: label.into(), a, b };
    let conditions = vec![
        c("Macchina", Some(ma.run.machine.clone()), Some(mb.run.machine.clone())),
        c("Build", Some(da.row.build.clone()), Some(db.row.build.clone())),
        c("Commit", ma.engine.commit.clone(), mb.engine.commit.clone()),
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
