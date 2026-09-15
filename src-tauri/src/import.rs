//! Importatore dei profili JSON di minis-config (`profiles/*.json`, letti da `serve.ps1`).
//!
//! I JSON non dettano il formato: si convertono nello schema TOML di Aethera. Le leve che in
//! minis-config stavano in `extra_args` e che hanno un campo proprio diventano quel campo.

use crate::profile::{self, Cache, ClientBudget, Model, Profile, Runtime, Sampling, Server, Speculative};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct MinisProfile {
    name: String,
    gate: Option<String>,
    model: MinisModel,
    runtime: MinisRuntime,
    server: MinisServer,
    sampling: Option<MinisSampling>,
    harness: Option<MinisHarness>,
    notes: Option<String>,
}

#[derive(Deserialize)]
struct MinisModel {
    repo: Option<String>,
    file: String,
    path: Option<String>,
    sha256: Option<String>,
    size_gb: Option<f64>,
    quant: Option<String>,
}

#[derive(Deserialize)]
struct MinisRuntime {
    kind: String,
    backend: String,
}

#[derive(Deserialize)]
struct MinisServer {
    host: String,
    port: u16,
    ctx: u32,
    n_parallel: u32,
    n_gpu_layers: i32,
    flash_attn: String,
    ubatch: u32,
    batch: u32,
    cache_type_k: String,
    cache_type_v: String,
    metrics: Option<bool>,
    jinja: Option<bool>,
    extra_args: Option<Vec<String>>,
    slot_save_path: Option<String>,
}

#[derive(Deserialize)]
struct MinisSampling {
    temperature: Option<f64>,
    top_p: Option<f64>,
    top_k: Option<i32>,
    min_p: Option<f64>,
    presence_penalty: Option<f64>,
    repeat_penalty: Option<f64>,
}

#[derive(Deserialize)]
struct MinisHarness {
    diorama_context_window: u32,
    reserved_output_tokens: u32,
}

#[derive(Debug, Serialize)]
pub struct Imported {
    pub profile: Profile,
    /// Cosa è stato convertito o lasciato indietro, da mostrare all'operatore.
    pub notes: Vec<String>,
}

fn value<'a>(args: &'a [String], i: usize, flag: &str) -> Result<&'a str, String> {
    args.get(i + 1).map(String::as_str).ok_or_else(|| format!("extra_args: {flag} senza valore"))
}

fn number<T: std::str::FromStr>(args: &[String], i: usize, flag: &str) -> Result<T, String> {
    let raw = value(args, i, flag)?;
    raw.parse().map_err(|_| format!("extra_args: valore di {flag} non numerico: «{raw}»"))
}

/// `build` è la build di llama.cpp da fissare nel profilo: il JSON non la porta (`runtime.version` è null).
pub fn import_minis_json(text: &str, source: &str, build: &str) -> Result<Imported, String> {
    // Set-Content -Encoding UTF8 di Windows PowerShell scrive il BOM.
    let text = text.trim_start_matches('\u{feff}');
    let m: MinisProfile = serde_json::from_str(text).map_err(|e| format!("{source}: JSON non leggibile: {e}"))?;
    let mut notes = Vec::new();

    let mut speculative = Speculative::default();
    let mut cache = Cache::default();
    let mut load_mode = "auto".to_string();
    let mut extra = Vec::new();
    let args = m.server.extra_args.unwrap_or_default();
    let mut i = 0;
    while i < args.len() {
        let flag = args[i].as_str();
        let consumed = match flag {
            "--spec-type" => {
                speculative.kind = value(&args, i, flag)?.to_string();
                2
            }
            "--spec-draft-n-max" => {
                speculative.draft_n_max = Some(number(&args, i, flag)?);
                2
            }
            "--spec-draft-n-min" => {
                speculative.draft_n_min = Some(number(&args, i, flag)?);
                2
            }
            "--spec-draft-p-min" | "--draft-p-min" => {
                speculative.draft_p_min = Some(number(&args, i, flag)?);
                2
            }
            "--spec-draft-model" | "-md" | "--model-draft" => {
                speculative.draft_model = Some(value(&args, i, flag)?.to_string());
                2
            }
            "--cache-reuse" => {
                cache.cache_reuse = Some(number(&args, i, flag)?);
                2
            }
            "--ctx-checkpoints" | "-ctxcp" | "--swa-checkpoints" => {
                cache.ctx_checkpoints = Some(number(&args, i, flag)?);
                2
            }
            "--load-mode" | "-lm" => {
                load_mode = value(&args, i, flag)?.to_string();
                2
            }
            other => {
                if let Some(field) = profile::managed_flag(other) {
                    return Err(format!("{source}: extra_args contiene «{other}», che in Aethera è {field}: da convertire a mano"));
                }
                extra.push(other.to_string());
                1
            }
        };
        if consumed == 2 {
            notes.push(format!("extra_args «{} {}» → campo proprio", args[i], args[i + 1]));
        }
        i += consumed;
    }

    if let Some(path) = &m.model.path {
        if path != &m.model.file {
            notes.push(format!("model.path «{path}» ignorato: i pesi si cercano per nome file nella cartella della macchina"));
        }
    }
    if m.server.slot_save_path.is_some() {
        notes.push("slot_save_path → slot_save = true (cartella runs/<id>/slots)".into());
    }
    notes.push("load_mode = auto, dichiarato (serve.ps1 non lo passava: era il default del motore)".into());

    let mut sampling_by_mode = BTreeMap::new();
    if let Some(s) = m.sampling {
        // Il JSON non dice per quale modalità vale: resta «declared», con la fonte.
        sampling_by_mode.insert(
            "declared".to_string(),
            Sampling {
                temperature: s.temperature,
                top_p: s.top_p,
                top_k: s.top_k,
                min_p: s.min_p,
                presence_penalty: s.presence_penalty,
                repeat_penalty: s.repeat_penalty,
                source: Some(format!("minis-config {source}")),
                verified: None,
            },
        );
    }

    let profile = Profile {
        schema_version: profile::SCHEMA_VERSION,
        name: m.name,
        gate: m.gate,
        notes: m.notes,
        model: Model {
            repo: m.model.repo,
            file: m.model.file,
            sha256: m.model.sha256.map(|h| h.to_ascii_lowercase()),
            size_gb: m.model.size_gb,
            quant: m.model.quant,
        },
        runtime: Runtime { kind: m.runtime.kind, backend: m.runtime.backend, build: build.to_string() },
        server: Server {
            host: m.server.host,
            port: m.server.port,
            ctx: m.server.ctx,
            n_parallel: m.server.n_parallel,
            n_gpu_layers: m.server.n_gpu_layers,
            flash_attn: m.server.flash_attn,
            cache_type_k: m.server.cache_type_k,
            cache_type_v: m.server.cache_type_v,
            ubatch: m.server.ubatch,
            batch: m.server.batch,
            load_mode,
            threads: None,
            threads_batch: None,
            metrics: m.server.metrics.unwrap_or(false),
            jinja: m.server.jinja.unwrap_or(false),
            slot_save: m.server.slot_save_path.is_some(),
            extra_args: extra,
        },
        speculative,
        cache,
        client: m.harness.map(|h| ClientBudget {
            context_window: h.diorama_context_window,
            reserved_output_tokens: h.reserved_output_tokens,
        }),
        sampling_by_mode,
    };
    Ok(Imported { profile, notes })
}
