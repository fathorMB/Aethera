//! Manifest dell'avvio in `runs/<id>/manifest.toml`: con che cosa è stato acceso il motore.
//! Si scrive all'avvio, si aggiorna quando il motore è pronto e quando esce.
//! Un campo assente è sconosciuto, mai zero.

use crate::memory::MemorySection;
use crate::profile::{Override, Profile};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const MANIFEST_SCHEMA: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Manifest {
    pub schema_version: u32,
    pub run: RunSection,
    pub engine: EngineSection,
    pub model: ModelSection,
    pub server: ServerSection,
    pub command: CommandSection,
    pub machine: MachineSection,
    /// Memoria prima dell'avvio e misurata dopo il caricamento (M-03): assente negli avvii precedenti.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<MemorySection>,
    #[serde(default, rename = "override", skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<Override>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<ExitSection>,
    /// Il profilo effettivo, già con le differenze: l'avvio resta riproducibile anche se il file cambia.
    pub effective_profile: Profile,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunSection {
    pub id: String,
    pub started: String,
    pub machine: String,
    pub profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_file: Option<String>,
    pub invalidates_cache: bool,
    pub log: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EngineSection {
    pub build_declared: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    pub backend: String,
    pub build_id: String,
    pub binary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelSection {
    pub file: String,
    pub path: String,
    pub size_bytes: u64,
    pub size_gb: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256_declared: Option<String>,
    /// Ricalcolato solo su richiesta (catalogo, M-04): assente finché non è stato fatto.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256_verified: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerSection {
    pub host: String,
    pub port: u16,
    pub alias: String,
    pub ctx_declared: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ctx_served: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias_served: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ready_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommandSection {
    pub line: String,
    pub argv: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MachineSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gpus: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ram_total_gib: Option<f64>,
    /// RAM disponibile letta subito prima dell'avvio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ram_available_gib_before: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExitSection {
    pub at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,
    pub by_user: bool,
    pub left_running: bool,
}

pub fn write(path: &Path, m: &Manifest) -> Result<(), String> {
    let text = toml::to_string_pretty(m).map_err(|e| format!("manifest: {e}"))?;
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn read(path: &Path) -> Result<Manifest, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}
