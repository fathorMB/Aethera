//! Schema TOML dei profili di Aethera (v1) e sua validazione.
//!
//! Un profilo descrive un avvio di `llama-server` senza percorsi assoluti: i pesi si trovano per
//! nome file nella cartella della macchina, il binario per build e backend. Il nome del file è il
//! nome del profilo ed è l'alias servito.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const SCHEMA_VERSION: u32 = 1;

pub const FLASH_ATTN: &[&str] = &["on", "off", "auto"];
pub const LOAD_MODES: &[&str] = &["auto", "none", "mmap", "mlock", "mmap+mlock", "dio"];
pub const CACHE_TYPES: &[&str] = &["f32", "f16", "bf16", "q8_0", "q4_0", "q4_1", "iq4_nl", "q5_0", "q5_1"];
pub const SPEC_TYPES: &[&str] = &[
    "none",
    "draft-simple",
    "draft-eagle3",
    "draft-mtp",
    "draft-dflash",
    "draft-dspark",
    "ngram-simple",
    "ngram-map-k",
    "ngram-map-k4v",
    "ngram-mod",
    "ngram-cache",
];
const SPEC_NEEDS_DRAFT_MODEL: &[&str] = &["draft-simple", "draft-eagle3", "draft-dflash", "draft-dspark"];

/// Campi che cambiano come il motore riusa il prefisso: una loro modifica si dichiara «invalida la cache».
pub const CACHE_FIELDS: &[&str] = &["server.ctx", "server.n_parallel", "cache.cache_reuse", "cache.ctx_checkpoints"];

fn yes() -> bool {
    true
}
fn auto() -> String {
    "auto".into()
}
fn none() -> String {
    "none".into()
}

// I campi scalari stanno prima delle tabelle: TOML li vuole in quest'ordine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema_version: u32,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub model: Model,
    pub runtime: Runtime,
    pub server: Server,
    #[serde(default)]
    pub speculative: Speculative,
    #[serde(default)]
    pub cache: Cache,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<ClientBudget>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sampling_by_mode: BTreeMap<String, Sampling>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Model {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    pub file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// GB decimali, come li scrive il publisher.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_gb: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quant: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Runtime {
    pub kind: String,
    pub backend: String,
    /// Build di llama.cpp scelta e fissata, per esempio `b10809`.
    pub build: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Server {
    pub host: String,
    pub port: u16,
    pub ctx: u32,
    pub n_parallel: u32,
    pub n_gpu_layers: i32,
    pub flash_attn: String,
    pub cache_type_k: String,
    pub cache_type_v: String,
    pub ubatch: u32,
    pub batch: u32,
    #[serde(default = "auto")]
    pub load_mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threads: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threads_batch: Option<i32>,
    #[serde(default = "yes")]
    pub metrics: bool,
    #[serde(default = "yes")]
    pub jinja: bool,
    /// Salvataggio degli slot in `runs/<id>/slots`: senza, `/slots?action=save|restore` non esiste.
    #[serde(default = "yes")]
    pub slot_save: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Speculative {
    #[serde(rename = "type", default = "none")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_n_max: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_n_min: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_p_min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_model: Option<String>,
}

impl Default for Speculative {
    fn default() -> Self {
        Self { kind: none(), draft_n_max: None, draft_n_min: None, draft_p_min: None, draft_model: None }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Cache {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_reuse: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ctx_checkpoints: Option<u32>,
}

/// Budget dichiarato dal client: finestra di conversazione + output riservato ≤ contesto.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ClientBudget {
    pub context_window: u32,
    pub reserved_output_tokens: u32,
}

/// Campionamento consigliato: dato del modello, non default del server.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Sampling {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_p: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat_penalty: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Issue {
    pub field: String,
    pub message: String,
}

fn issue(field: &str, message: impl Into<String>) -> Issue {
    Issue { field: field.into(), message: message.into() }
}

/// Argomenti che hanno un campo proprio: in `extra_args` sarebbero una leva nascosta o un doppione.
const MANAGED_FLAGS: &[(&str, &str)] = &[
    ("-m", "model.file"),
    ("--model", "model.file"),
    ("--host", "server.host"),
    ("--port", "server.port"),
    ("-c", "server.ctx"),
    ("--ctx-size", "server.ctx"),
    ("-np", "server.n_parallel"),
    ("--parallel", "server.n_parallel"),
    ("-ngl", "server.n_gpu_layers"),
    ("--n-gpu-layers", "server.n_gpu_layers"),
    ("--gpu-layers", "server.n_gpu_layers"),
    ("-ub", "server.ubatch"),
    ("--ubatch-size", "server.ubatch"),
    ("-b", "server.batch"),
    ("--batch-size", "server.batch"),
    ("-fa", "server.flash_attn"),
    ("--flash-attn", "server.flash_attn"),
    ("-ctk", "server.cache_type_k"),
    ("--cache-type-k", "server.cache_type_k"),
    ("-ctv", "server.cache_type_v"),
    ("--cache-type-v", "server.cache_type_v"),
    ("-a", "name"),
    ("--alias", "name"),
    ("--slot-save-path", "server.slot_save"),
    ("--metrics", "server.metrics"),
    ("--jinja", "server.jinja"),
    ("--no-jinja", "server.jinja"),
    ("-lm", "server.load_mode"),
    ("--load-mode", "server.load_mode"),
    ("--mmap", "server.load_mode"),
    ("--no-mmap", "server.load_mode"),
    ("--mlock", "server.load_mode"),
    ("-dio", "server.load_mode"),
    ("--direct-io", "server.load_mode"),
    ("-ndio", "server.load_mode"),
    ("--no-direct-io", "server.load_mode"),
    ("-t", "server.threads"),
    ("--threads", "server.threads"),
    ("-tb", "server.threads_batch"),
    ("--threads-batch", "server.threads_batch"),
    ("--spec-type", "speculative.type"),
    ("--spec-draft-n-max", "speculative.draft_n_max"),
    ("--spec-draft-n-min", "speculative.draft_n_min"),
    ("--spec-draft-p-min", "speculative.draft_p_min"),
    ("--draft-p-min", "speculative.draft_p_min"),
    ("--spec-draft-model", "speculative.draft_model"),
    ("-md", "speculative.draft_model"),
    ("--model-draft", "speculative.draft_model"),
    ("--cache-reuse", "cache.cache_reuse"),
    ("-ctxcp", "cache.ctx_checkpoints"),
    ("--ctx-checkpoints", "cache.ctx_checkpoints"),
    ("--swa-checkpoints", "cache.ctx_checkpoints"),
];

pub fn managed_flag(token: &str) -> Option<&'static str> {
    let flag = token.split('=').next().unwrap_or(token);
    MANAGED_FLAGS.iter().find(|(f, _)| *f == flag).map(|(_, field)| *field)
}

fn one_of(field: &str, value: &str, allowed: &[&str], v: &mut Vec<Issue>) {
    if !allowed.contains(&value) {
        v.push(issue(field, format!("«{value}» non ammesso: {}", allowed.join(" · "))));
    }
}

fn file_name_only(field: &str, file: &str, v: &mut Vec<Issue>) {
    if file.trim().is_empty() || file.contains(['/', '\\']) {
        v.push(issue(field, "solo il nome del file, senza percorso: la cartella dei pesi è della macchina"));
    } else if !file.to_ascii_lowercase().ends_with(".gguf") {
        v.push(issue(field, "atteso un file .gguf"));
    }
}

/// Legge un profilo dal testo TOML. Restituisce il profilo quando è leggibile, anche se ha errori
/// di validazione, così la UI può mostrarlo e correggerlo.
pub fn load(text: &str, stem: &str) -> (Option<Profile>, Vec<Issue>) {
    let p: Profile = match toml::from_str(text) {
        Ok(p) => p,
        Err(e) => return (None, vec![issue("", e.to_string())]),
    };
    let mut issues = validate(&p);
    if p.name != stem {
        issues.insert(
            0,
            issue(
                "name",
                format!("il nome «{}» non coincide con il file «{stem}.toml»: alias e manifest citano il profilo per nome", p.name),
            ),
        );
    }
    (Some(p), issues)
}

pub fn validate(p: &Profile) -> Vec<Issue> {
    let mut v = Vec::new();

    if p.schema_version != SCHEMA_VERSION {
        v.push(issue(
            "schema_version",
            format!("versione {} non supportata: questa Aethera legge la {SCHEMA_VERSION}", p.schema_version),
        ));
    }
    if p.name.is_empty() || !p.name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')) {
        v.push(issue("name", "nome vuoto o con caratteri non ammessi (lettere, cifre, . - _)"));
    }

    file_name_only("model.file", &p.model.file, &mut v);
    if let Some(h) = &p.model.sha256 {
        if h.len() != 64 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
            v.push(issue("model.sha256", "atteso uno SHA-256 di 64 cifre esadecimali"));
        }
    }
    if let Some(s) = p.model.size_gb {
        if !(s > 0.0) {
            v.push(issue("model.size_gb", "deve essere maggiore di zero (GB decimali del publisher)"));
        }
    }

    if p.runtime.kind.trim().is_empty() {
        v.push(issue("runtime.kind", "obbligatorio (llama.cpp)"));
    }
    if p.runtime.backend.trim().is_empty() {
        v.push(issue("runtime.backend", "obbligatorio (per esempio vulkan)"));
    }
    if p.runtime.build.trim().is_empty() {
        v.push(issue("runtime.build", "obbligatorio: il motore è scelto e fissato (per esempio b10809)"));
    }

    let s = &p.server;
    if s.host.trim().is_empty() {
        v.push(issue("server.host", "obbligatorio"));
    }
    if s.port == 0 {
        v.push(issue("server.port", "porta esplicita obbligatoria: il default di llama-server cambierà"));
    }
    if s.ctx < 256 {
        v.push(issue("server.ctx", "contesto troppo piccolo (minimo 256)"));
    }
    if s.n_parallel == 0 {
        v.push(issue("server.n_parallel", "almeno 1 slot"));
    }
    one_of("server.flash_attn", &s.flash_attn, FLASH_ATTN, &mut v);
    one_of("server.cache_type_k", &s.cache_type_k, CACHE_TYPES, &mut v);
    one_of("server.cache_type_v", &s.cache_type_v, CACHE_TYPES, &mut v);
    one_of("server.load_mode", &s.load_mode, LOAD_MODES, &mut v);
    if s.ubatch == 0 || s.batch == 0 {
        v.push(issue("server.ubatch", "ubatch e batch devono essere maggiori di zero"));
    } else if s.ubatch > s.batch {
        v.push(issue("server.ubatch", "ubatch maggiore di batch: llama.cpp lo ridurrebbe in silenzio"));
    }
    for (field, t) in [("server.threads", s.threads), ("server.threads_batch", s.threads_batch)] {
        if matches!(t, Some(n) if n == 0 || n < -1) {
            v.push(issue(field, "atteso -1 (automatico) o un numero positivo"));
        }
    }
    for a in &s.extra_args {
        if let Some(field) = managed_flag(a) {
            v.push(issue("server.extra_args", format!("«{a}» ha un campo proprio: usa {field}")));
        }
    }

    let sp = &p.speculative;
    let kinds: Vec<&str> = sp.kind.split(',').map(str::trim).collect();
    for k in &kinds {
        one_of("speculative.type", k, SPEC_TYPES, &mut v);
    }
    let active = !(kinds.len() == 1 && kinds[0] == "none");
    if kinds.len() > 1 && kinds.contains(&"none") {
        v.push(issue("speculative.type", "none non si combina con altri tipi"));
    }
    if !active
        && (sp.draft_n_max.is_some() || sp.draft_n_min.is_some() || sp.draft_p_min.is_some() || sp.draft_model.is_some())
    {
        v.push(issue("speculative.type", "leve di speculazione impostate con type = none: non avrebbero effetto"));
    }
    if kinds.contains(&"draft-mtp") && s.n_parallel != 1 {
        v.push(issue("server.n_parallel", "la decodifica MTP supporta solo n_parallel = 1"));
    }
    if kinds.iter().any(|k| SPEC_NEEDS_DRAFT_MODEL.contains(k)) && sp.draft_model.is_none() {
        v.push(issue("speculative.draft_model", "questo tipo di speculazione richiede un modello draft"));
    }
    if let (Some(mn), Some(mx)) = (sp.draft_n_min, sp.draft_n_max) {
        if mn > mx {
            v.push(issue("speculative.draft_n_min", "maggiore di draft_n_max"));
        }
    }
    if matches!(sp.draft_p_min, Some(x) if !(0.0..=1.0).contains(&x)) {
        v.push(issue("speculative.draft_p_min", "atteso fra 0 e 1"));
    }
    if let Some(f) = &sp.draft_model {
        file_name_only("speculative.draft_model", f, &mut v);
    }

    if let Some(c) = &p.client {
        if c.context_window as u64 + c.reserved_output_tokens as u64 > s.ctx as u64 {
            v.push(issue(
                "client.context_window",
                format!(
                    "finestra {} + output riservato {} supera il contesto del server {}",
                    c.context_window, c.reserved_output_tokens, s.ctx
                ),
            ));
        }
    }

    for (mode, sm) in &p.sampling_by_mode {
        let field = format!("sampling_by_mode.{mode}");
        if mode.is_empty() || !mode.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
            v.push(issue(&field, "nome di modalità: minuscole, cifre e _"));
        }
        if matches!(sm.temperature, Some(t) if t < 0.0) {
            v.push(issue(&format!("{field}.temperature"), "non può essere negativa"));
        }
        for (name, x) in [("top_p", sm.top_p), ("min_p", sm.min_p)] {
            if matches!(x, Some(x) if !(0.0..=1.0).contains(&x)) {
                v.push(issue(&format!("{field}.{name}"), "atteso fra 0 e 1"));
            }
        }
    }

    v
}

/// Una differenza fra il profilo base e quello avviato: entra nel manifest come override.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Override {
    pub field: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

pub fn diff(base: &Profile, edited: &Profile) -> Vec<Override> {
    let mut a = BTreeMap::new();
    let mut b = BTreeMap::new();
    flatten("", &serde_json::to_value(base).unwrap_or(Value::Null), &mut a);
    flatten("", &serde_json::to_value(edited).unwrap_or(Value::Null), &mut b);
    let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .filter(|k| a.get(*k) != b.get(*k))
        .map(|k| Override { field: k.clone(), base: a.get(k).map(render), value: b.get(k).map(render) })
        .collect()
}

fn flatten(prefix: &str, v: &Value, out: &mut BTreeMap<String, Value>) {
    match v {
        Value::Object(map) => {
            for (k, x) in map {
                let key = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                flatten(&key, x, out);
            }
        }
        Value::Null => {}
        other => {
            out.insert(prefix.to_string(), other.clone());
        }
    }
}

fn render(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
schema_version = 1
name = "prova"

[model]
file = "prova.gguf"

[runtime]
kind = "llama.cpp"
backend = "vulkan"
build = "b10809"

[server]
host = "127.0.0.1"
port = 8081
ctx = 32768
n_parallel = 1
n_gpu_layers = 999
flash_attn = "on"
cache_type_k = "f16"
cache_type_v = "f16"
ubatch = 4096
batch = 4096
"#;

    #[test]
    fn minimal_profile_is_valid_with_explicit_defaults() {
        let (p, issues) = load(MINIMAL, "prova");
        assert!(issues.is_empty(), "{issues:?}");
        let p = p.unwrap();
        assert_eq!(p.server.load_mode, "auto");
        assert_eq!(p.speculative.kind, "none");
        assert!(p.server.metrics && p.server.jinja && p.server.slot_save);
    }

    #[test]
    fn unknown_field_is_a_readable_error() {
        let (p, issues) = load(&MINIMAL.replace("ubatch = 4096", "ubatch = 4096\nubach = 1"), "prova");
        assert!(p.is_none());
        assert!(issues[0].message.contains("ubach"), "{issues:?}");
    }

    #[test]
    fn reports_every_semantic_error() {
        let text = MINIMAL
            .replace("name = \"prova\"", "name = \"altro\"")
            .replace("n_parallel = 1", "n_parallel = 2")
            .replace("ubatch = 4096", "ubatch = 8192")
            .replace("flash_attn = \"on\"", "flash_attn = \"si\"")
            + "extra_args = [\"--no-mmap\"]\n\n[speculative]\ntype = \"draft-mtp\"\n";
        let (_, issues) = load(&text, "prova");
        let fields: Vec<&str> = issues.iter().map(|i| i.field.as_str()).collect();
        for f in ["name", "server.flash_attn", "server.ubatch", "server.extra_args", "server.n_parallel"] {
            assert!(fields.contains(&f), "manca {f} in {issues:?}");
        }
    }

    #[test]
    fn diff_lists_changed_fields() {
        let (base, _) = load(MINIMAL, "prova");
        let base = base.unwrap();
        let mut edited = base.clone();
        edited.server.ubatch = 2048;
        edited.speculative.draft_n_max = Some(4);
        edited.speculative.kind = "draft-mtp".into();
        let d = diff(&base, &edited);
        assert_eq!(
            d,
            vec![
                Override { field: "server.ubatch".into(), base: Some("4096".into()), value: Some("2048".into()) },
                Override { field: "speculative.draft_n_max".into(), base: None, value: Some("4".into()) },
                Override { field: "speculative.type".into(), base: Some("none".into()), value: Some("draft-mtp".into()) },
            ]
        );
    }
}
