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

pub const ON_OFF: &[&str] = &["on", "off"];
pub const LAZY_MODES: &[&str] = &["auto", "on", "off"];

/// Campi che cambiano come il motore riusa il prefisso: una loro modifica si dichiara «invalida la cache».
pub const CACHE_FIELDS: &[&str] = &[
    "server.ctx",
    "server.n_parallel",
    "cache.cache_reuse",
    "cache.ctx_checkpoints",
    "cache.checkpoint_min_step",
    "cache.cache_ram",
    "cache.kv_unified",
];

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
    /// Assente: i layer sulla GPU li sceglie `--fit`, che regola solo gli argomenti non impostati.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n_gpu_layers: Option<i32>,
    /// `on` o `off`: il default del motore (b10991) è `on`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit: Option<String>,
    /// MiB da lasciare liberi per dispositivo, separati da virgole: `1024` o `1024,512`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit_target: Option<String>,
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
    /// Esperti MoE dei primi N layer tenuti sulla CPU. M-08 T-08: su questa macchina peggiora sempre.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n_cpu_moe: Option<u32>,
    /// `-ot`: una regola `regex=buffer` per voce.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tensor_overrides: Vec<String>,
    /// `--lazy-mode`: `auto`, `on`, `off`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lazy_mode: Option<String>,
    /// Template di chat al posto di quello del modello: nome di un file in `<radice>/templates`.
    /// Aethera vi scrive `qwen3.6-tollerante.jinja`, che serve a Claude Code (M-09).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_template_file: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_min_step: Option<u32>,
    /// MiB di cache dei prompt in RAM: -1 senza limite, 0 disattivata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_ram: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kv_unified: Option<bool>,
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

/// Un profilo nuovo con valori sensati e **tutti espliciti**: nessun default del motore lasciato
/// implicito, così la riga di comando dice per intero come è stato avviato. Il nome è anche
/// l'alias servito; porta e build le sceglie chi chiama, guardando la macchina.
pub fn template(name: &str, model_file: &str, build: &str, backend: &str, port: u16) -> Profile {
    Profile {
        schema_version: SCHEMA_VERSION,
        name: name.to_string(),
        gate: None,
        notes: None,
        model: Model { repo: None, file: model_file.to_string(), sha256: None, size_gb: None, quant: None },
        runtime: Runtime { kind: "llama.cpp".into(), backend: backend.to_string(), build: build.to_string() },
        server: Server {
            host: "127.0.0.1".into(),
            port,
            ctx: 32768,
            n_parallel: 1,
            n_gpu_layers: Some(999),
            fit: None,
            fit_target: None,
            flash_attn: "on".into(),
            cache_type_k: "f16".into(),
            cache_type_v: "f16".into(),
            ubatch: 2048,
            batch: 2048,
            load_mode: auto(),
            threads: None,
            threads_batch: None,
            n_cpu_moe: None,
            tensor_overrides: Vec::new(),
            lazy_mode: None,
            chat_template_file: None,
            metrics: true,
            jinja: true,
            slot_save: true,
            extra_args: Vec::new(),
        },
        speculative: Speculative::default(),
        cache: Cache::default(),
        client: None,
        sampling_by_mode: BTreeMap::new(),
    }
}

/// Nome di profilo ricavato dal file dei pesi: minuscole e soli caratteri ammessi dallo schema.
pub fn name_from_file(file: &str) -> String {
    let stem = file.trim_end_matches(".gguf").trim_end_matches(".GGUF");
    let mut out = String::new();
    let mut last_dash = true;
    for c in stem.chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
            out.push(c);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "profilo".into()
    } else {
        out
    }
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
    ("-fit", "server.fit"),
    ("--fit", "server.fit"),
    ("-fitt", "server.fit_target"),
    ("--fit-target", "server.fit_target"),
    ("-ncmoe", "server.n_cpu_moe"),
    ("--n-cpu-moe", "server.n_cpu_moe"),
    ("-ot", "server.tensor_overrides"),
    ("--override-tensor", "server.tensor_overrides"),
    ("-lzm", "server.lazy_mode"),
    ("--lazy-mode", "server.lazy_mode"),
    ("--chat-template-file", "server.chat_template_file"),
    ("--chat-template", "server.chat_template_file"),
    ("-cms", "cache.checkpoint_min_step"),
    ("--checkpoint-min-step", "cache.checkpoint_min_step"),
    ("-cram", "cache.cache_ram"),
    ("--cache-ram", "cache.cache_ram"),
    ("-kvu", "cache.kv_unified"),
    ("--kv-unified", "cache.kv_unified"),
    ("-no-kvu", "cache.kv_unified"),
    ("--no-kv-unified", "cache.kv_unified"),
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
    if let Some(f) = &s.fit {
        one_of("server.fit", f, ON_OFF, &mut v);
    }
    if s.n_gpu_layers.is_none() && s.fit.as_deref() == Some("off") {
        v.push(issue(
            "server.n_gpu_layers",
            "senza n_gpu_layers e con fit = off i layer sulla GPU restano al default del motore: una leva nascosta",
        ));
    }
    if let Some(t) = &s.fit_target {
        if t.split(',').any(|x| x.trim().parse::<u32>().is_err()) {
            v.push(issue("server.fit_target", "attesi MiB interi separati da virgole, per esempio 1024 o 1024,512"));
        }
        if s.fit.as_deref() == Some("off") {
            v.push(issue("server.fit_target", "con fit = off non ha effetto"));
        }
    }
    if let Some(l) = &s.lazy_mode {
        one_of("server.lazy_mode", l, LAZY_MODES, &mut v);
        if l == "on" && !s.load_mode.contains("mmap") && s.load_mode != "auto" {
            v.push(issue("server.lazy_mode", "lazy_mode = on legge dal disco su richiesta: richiede load_mode con mmap"));
        }
    }
    for (i, o) in s.tensor_overrides.iter().enumerate() {
        match o.split_once('=') {
            Some((pat, buf)) if !pat.trim().is_empty() && !buf.trim().is_empty() && !o.contains(',') => {}
            _ => v.push(issue(
                "server.tensor_overrides",
                format!("regola {} «{o}»: attesa una sola regola `regex=buffer`, senza virgole", i + 1),
            )),
        }
    }
    if let Some(t) = &s.chat_template_file {
        if t.trim().is_empty() || t.contains(['/', '\\']) {
            v.push(issue("server.chat_template_file", "solo il nome del file: i template stanno in <radice dati>/templates"));
        } else if !t.to_ascii_lowercase().ends_with(".jinja") {
            v.push(issue("server.chat_template_file", "atteso un file .jinja"));
        }
    }
    if matches!(p.cache.cache_ram, Some(n) if n < -1) {
        v.push(issue("cache.cache_ram", "atteso -1 (senza limite), 0 (disattivata) o MiB"));
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
    fn measured_levers_have_their_own_fields() {
        let text = MINIMAL.replace("n_gpu_layers = 999\n", "fit = \"on\"\nfit_target = \"1024,512\"\n")
            + "n_cpu_moe = 8\ntensor_overrides = [\"blk\\\\.1\\\\.ffn_.*=CPU\"]\nlazy_mode = \"auto\"\nchat_template_file = \"qwen3.6-tollerante.jinja\"\n\n[cache]\ncheckpoint_min_step = 128\ncache_ram = 15000\nkv_unified = true\n";
        let (p, issues) = load(&text, "prova");
        assert!(issues.is_empty(), "{issues:?}");
        let p = p.unwrap();
        assert_eq!((p.server.n_gpu_layers, p.server.fit.as_deref()), (None, Some("on")));
        assert_eq!(p.server.tensor_overrides, vec![r"blk\.1\.ffn_.*=CPU".to_string()]);
        assert_eq!((p.cache.cache_ram, p.cache.kv_unified), (Some(15000), Some(true)));
        let back = toml::to_string_pretty(&p).unwrap();
        assert_eq!(load(&back, "prova").0.unwrap(), p);
    }

    #[test]
    fn a_hidden_lever_is_an_error() {
        let text = MINIMAL.replace("n_gpu_layers = 999\n", "fit = \"off\"\nfit_target = \"mille\"\n")
            + "tensor_overrides = [\"a=CPU,b=CPU\", \"senza-uguale\"]\nlazy_mode = \"sempre\"\nchat_template_file = \"C:\\\\t.jinja\"\nextra_args = [\"--n-cpu-moe\", \"4\", \"-kvu\"]\n\n[cache]\ncache_ram = -5\n";
        let (_, issues) = load(&text, "prova");
        let fields: Vec<&str> = issues.iter().map(|i| i.field.as_str()).collect();
        for f in [
            "server.n_gpu_layers",
            "server.fit_target",
            "server.lazy_mode",
            "server.tensor_overrides",
            "server.chat_template_file",
            "cache.cache_ram",
            "server.extra_args",
        ] {
            assert!(fields.contains(&f), "manca {f} in {issues:?}");
        }
        assert_eq!(fields.iter().filter(|f| **f == "server.tensor_overrides").count(), 2);
        assert_eq!(fields.iter().filter(|f| **f == "server.extra_args").count(), 2);
    }

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
    fn a_new_profile_is_valid_and_serves_its_own_name_as_alias() {
        let p = template("nuovo.q4", "Modello-Q4_K_M.gguf", "b10809", "vulkan", 8081);
        assert!(validate(&p).is_empty(), "{:?}", validate(&p));
        let text = toml::to_string_pretty(&p).unwrap();
        let (back, issues) = load(&text, "nuovo.q4");
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(back.unwrap(), p);
    }

    #[test]
    fn profile_name_from_weights_file_keeps_only_allowed_characters() {
        assert_eq!(name_from_file("Qwen_Qwen3.6-35B-A3B-Q4_K_M.gguf"), "qwen_qwen3.6-35b-a3b-q4_k_m");
        assert_eq!(name_from_file("modello (copia 2).gguf"), "modello-copia-2");
        assert_eq!(name_from_file("!!!.gguf"), "profilo");
        // Il nome ricavato deve passare la validazione: è anche l'alias servito.
        let p = template(&name_from_file("Qwen_Qwen3.6-35B-A3B-Q4_K_M.gguf"), "x.gguf", "b1", "vulkan", 8080);
        assert!(validate(&p).is_empty());
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
