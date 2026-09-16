//! M-08: banco di misura del motore. Ogni variante è **un avvio di Aethera da profilo**
//! (`launch::prepare` + `Engine::start`), quindi lascia il suo `runs/<id>` con manifest, log e
//! telemetria: il run id è la prova della misura.
//!
//! Uso: `cargo run --release --example m08_bench -- <radice> <scenario.toml> [--dry]`
//!
//! Lo scenario dichiara il profilo di partenza, le varianti (una variabile per volta) e i carichi.
//! Il banco non giudica: scrive `m08/<misura>.jsonl` con una riga per giro, **numeri grezzi del
//! motore** (il blocco `timings` della risposta così com'è), e stampa le mediane.
//!
//! Regole che il banco fa rispettare da solo:
//! - un giro di riscaldamento non contato prima dei giri buoni, così il primo prefill non pesa;
//! - cache fredda quando il carico lo chiede (nonce in testa: `cache_prompt` resta acceso perché è
//!   la condizione vera d'esercizio, ma il prefisso è nuovo);
//! - `n_prompt_tokens_cache` letto da `/slots` mentre la richiesta gira, non da `/metrics`
//!   (il contatore `…_cached_total` sale all'avvio della richiesta dopo, non di questa).

use aethera_lib::engine::{Engine, EngineStatus, StartRequest};
use aethera_lib::machine::DataRoot;
use aethera_lib::profile::Profile;
use aethera_lib::{launch, manifest, system, telemetry};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------- lo scenario

#[derive(Debug, Deserialize)]
struct Scenario {
    measure: String,
    title: String,
    base: String,
    #[serde(default = "five")]
    repetitions: usize,
    /// Giri di riscaldamento non contati, per carico.
    #[serde(default = "one")]
    warmup: usize,
    variant: Vec<Variant>,
    workload: Vec<Workload>,
}

fn five() -> usize {
    5
}
fn one() -> usize {
    1
}

#[derive(Debug, Deserialize)]
struct Variant {
    name: String,
    #[serde(default)]
    note: Option<String>,
    /// Profilo diverso da `base` per questa variante (per esempio un altro modello).
    #[serde(default)]
    profile: Option<String>,
    #[serde(default)]
    build: Option<String>,
    #[serde(default)]
    extra_args: Vec<String>,
    #[serde(default)]
    server: ServerPatch,
    #[serde(default)]
    speculative: SpecPatch,
    #[serde(default)]
    cache: CachePatch,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ServerPatch {
    ctx: Option<u32>,
    ubatch: Option<u32>,
    batch: Option<u32>,
    n_parallel: Option<u32>,
    n_gpu_layers: Option<i32>,
    flash_attn: Option<String>,
    cache_type_k: Option<String>,
    cache_type_v: Option<String>,
    load_mode: Option<String>,
    /// M-10: `--lazy-mode` e le regole `-ot` (la tabella n-gram lasciata sul file).
    lazy_mode: Option<String>,
    tensor_overrides: Option<Vec<String>>,
    fit: Option<String>,
    fit_target: Option<String>,
    threads: Option<i32>,
    threads_batch: Option<i32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct SpecPatch {
    #[serde(rename = "type")]
    kind: Option<String>,
    draft_n_max: Option<u32>,
    draft_n_min: Option<u32>,
    draft_p_min: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct CachePatch {
    cache_reuse: Option<u32>,
    ctx_checkpoints: Option<u32>,
}

#[derive(Debug, Deserialize, Clone)]
struct Workload {
    name: String,
    /// `single` una richiesta sola; `turns` una sequenza con prefisso che diverge a metà.
    #[serde(default = "single")]
    kind: String,
    prompt: String,
    #[serde(default)]
    instruction: String,
    #[serde(default = "n384")]
    n_predict: u32,
    #[serde(default)]
    temperature: f64,
    #[serde(default)]
    top_p: Option<f64>,
    #[serde(default)]
    top_k: Option<u32>,
    #[serde(default)]
    min_p: Option<f64>,
    #[serde(default)]
    presence_penalty: Option<f64>,
    #[serde(default)]
    seed: Option<i64>,
    /// Nonce in testa: il prefisso non è mai già in cache.
    #[serde(default = "yes")]
    cold: bool,
    /// Solo per `turns`: quanti turni, e a che frazione del prompt cade la modifica.
    #[serde(default = "four")]
    turns: usize,
    #[serde(default = "half")]
    divergence_at: f64,
}

fn single() -> String {
    "single".into()
}
fn n384() -> u32 {
    384
}
fn yes() -> bool {
    true
}
fn four() -> usize {
    4
}
fn half() -> f64 {
    0.5
}

// ---------------------------------------------------------------- una riga di risultato

#[derive(Debug, Serialize)]
struct Row {
    measure: String,
    variant: String,
    workload: String,
    run_id: String,
    turn: usize,
    rep: usize,
    warmup: bool,
    at: String,
    /// `timings` del motore, così come lo manda: prompt_n, prompt_ms, predicted_*, draft_*…
    timings: Value,
    tokens_evaluated: Option<u64>,
    tokens_predicted: Option<u64>,
    /// `tokens_cached` della risposta: quanti token del prompt il motore ha riusato.
    tokens_cached: Option<u64>,
    /// Massimo `n_prompt_tokens_cache` visto su `/slots` mentre la richiesta girava.
    cache_tokens: Option<u32>,
    /// Attesa misurata dal client: dall'invio alla risposta completa.
    wall_ms: u64,
    prompt_tokens_sent: Option<u64>,
    text_sha256_12: String,
    text_head: String,
    stop_reason: Option<String>,
}

// ---------------------------------------------------------------- HTTP

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(1800)))
        .http_status_as_error(false)
        .build()
        .into()
}

fn post_json(base_url: &str, path: &str, body: Value) -> Result<Value, String> {
    let url = format!("{base_url}{path}");
    let mut r = agent().post(&url).send_json(&body).map_err(|e| format!("{url}: {e}"))?;
    let status = r.status().as_u16();
    let v: Value = r.body_mut().read_json().map_err(|e| format!("{url}: risposta {status}: {e}"))?;
    if status != 200 {
        return Err(format!("{url}: risposta {status}: {v}"));
    }
    Ok(v)
}

fn get_json(base_url: &str, path: &str) -> Option<Value> {
    let mut r = agent().get(&format!("{base_url}{path}")).call().ok()?;
    r.body_mut().read_json().ok()
}

/// Guarda `/slots` mentre la richiesta gira: a slot libero i contatori tornano a zero, quindi si
/// tiene il massimo visto con uno slot che sta lavorando.
fn watch_slots(base_url: &str, stop: Arc<AtomicBool>, seen: Arc<AtomicU32>) {
    let url = base_url.to_string();
    std::thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            if let Some(v) = get_json(&url, "/slots") {
                for (id_task, cached) in telemetry::slot_caches(&v) {
                    if id_task >= 0 && cached > seen.load(Ordering::Relaxed) {
                        seen.store(cached, Ordering::Relaxed);
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(80));
        }
    });
}

fn sha12(text: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    format!("{:x}", h.finalize())[..12].to_string()
}

// ---------------------------------------------------------------- il carico

fn build_body(w: &Workload, prompt: &str, n_predict: u32) -> Value {
    let mut b = json!({
        "prompt": prompt,
        "n_predict": n_predict,
        "cache_prompt": true,
        "temperature": w.temperature,
        "timings_per_token": false,
    });
    let o = b.as_object_mut().unwrap();
    if let Some(x) = w.top_p {
        o.insert("top_p".into(), json!(x));
    }
    if let Some(x) = w.top_k {
        o.insert("top_k".into(), json!(x));
    }
    if let Some(x) = w.min_p {
        o.insert("min_p".into(), json!(x));
    }
    if let Some(x) = w.presence_penalty {
        o.insert("presence_penalty".into(), json!(x));
    }
    if let Some(x) = w.seed {
        o.insert("seed".into(), json!(x));
    }
    b
}

/// Il prompt vero che vede il motore: il contenuto passa per il template del modello, con il
/// ragionamento esplicito **spento** (`enable_thinking: false`), che è la modalità dichiarata dal
/// profilo G1. Senza template un modello istruito risponde con un EOS e basta: si misurerebbe il
/// prefill e nient'altro.
fn templated(base_url: &str, content: &str) -> Result<String, String> {
    let v = post_json(
        base_url,
        "/apply-template",
        json!({
            "messages": [{"role": "user", "content": content}],
            "chat_template_kwargs": {"enable_thinking": false}
        }),
    )?;
    v["prompt"].as_str().map(str::to_string).ok_or_else(|| format!("/apply-template: risposta inattesa: {v}"))
}

#[allow(clippy::too_many_arguments)]
fn one_request(
    base_url: &str,
    w: &Workload,
    prompt: &str,
    measure: &str,
    variant: &str,
    run_id: &str,
    turn: usize,
    rep: usize,
    warmup: bool,
) -> Result<Row, String> {
    let prompt = templated(base_url, prompt)?;
    let stop = Arc::new(AtomicBool::new(false));
    let seen = Arc::new(AtomicU32::new(0));
    watch_slots(base_url, stop.clone(), seen.clone());
    let t0 = Instant::now();
    let v = post_json(base_url, "/completion", build_body(w, &prompt, w.n_predict));
    let wall_ms = t0.elapsed().as_millis() as u64;
    stop.store(true, Ordering::Relaxed);
    let v = v?;
    let content = v["content"].as_str().unwrap_or_default();
    let cache_tokens = match seen.load(Ordering::Relaxed) {
        0 => None,
        n => Some(n),
    };
    Ok(Row {
        measure: measure.into(),
        variant: variant.into(),
        workload: w.name.clone(),
        run_id: run_id.into(),
        turn,
        rep,
        warmup,
        at: chrono::Local::now().to_rfc3339(),
        timings: v["timings"].clone(),
        tokens_evaluated: v["tokens_evaluated"].as_u64(),
        tokens_predicted: v["tokens_predicted"].as_u64(),
        tokens_cached: v["tokens_cached"].as_u64(),
        cache_tokens,
        wall_ms,
        prompt_tokens_sent: v["timings"]["prompt_n"].as_u64(),
        text_sha256_12: sha12(content),
        text_head: content.chars().take(160).collect(),
        stop_reason: v["stop_type"].as_str().map(str::to_string),
    })
}

/// Il prompt di un turno: nonce (se richiesto), corpo, eventuale modifica a metà, istruzione.
///
/// Il nonce è **uno per giro**, non uno per turno: dentro un giro i turni devono condividere il
/// prefisso (è quello che si sta misurando), fra un giro e l'altro no (cache fredda).
fn turn_prompt(w: &Workload, body: &str, nonce: &str, turn: usize) -> String {
    let nonce = if w.cold { format!("// banco M-08 · {nonce}\n") } else { String::new() };
    let body = if turn >= 2 && w.kind == "turns" {
        // Turno divergente: una riga cambia **in mezzo** al prompt, come quando un agente
        // riscrive un file già in conversazione. Tutto ciò che sta dopo va rielaborato, a meno
        // che il motore non sappia ripartire da un checkpoint.
        let cut = (body.len() as f64 * w.divergence_at) as usize;
        let cut = body[..cut.min(body.len())].rfind('\n').unwrap_or(0);
        format!(
            "{}\n// --- modifica del turno {turn}: questa riga non c'era ({}) ---{}",
            &body[..cut],
            turn * 7919,
            &body[cut..]
        )
    } else {
        body.to_string()
    };
    let extra = if turn >= 1 {
        format!("\n\n(turno {turn} della stessa conversazione)")
    } else {
        String::new()
    };
    format!("{nonce}{body}\n\n{}{extra}\n", w.instruction)
}

// ---------------------------------------------------------------- mediane

fn med(v: &[f64]) -> Option<f64> {
    telemetry::median(v)
}

fn spread(v: &[f64]) -> Option<f64> {
    if v.len() < 2 {
        return None;
    }
    let m = v.iter().sum::<f64>() / v.len() as f64;
    Some((v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (v.len() - 1) as f64).sqrt())
}

// ---------------------------------------------------------------- avvio di una variante

struct Started {
    run_id: String,
    base_url: String,
    manifest_path: PathBuf,
    load_ms: u64,
}

fn patch(p: &mut Profile, v: &Variant) {
    let s = &mut p.server;
    macro_rules! set {
        ($f:ident) => {
            if let Some(x) = v.server.$f.clone() {
                s.$f = x;
            }
        };
        (opt $f:ident) => {
            if let Some(x) = v.server.$f {
                s.$f = Some(x);
            }
        };
    }
    set!(ctx);
    set!(ubatch);
    set!(batch);
    set!(n_parallel);
    set!(opt n_gpu_layers);
    set!(flash_attn);
    set!(cache_type_k);
    set!(cache_type_v);
    set!(load_mode);
    set!(tensor_overrides);
    if v.server.lazy_mode.is_some() {
        s.lazy_mode = v.server.lazy_mode.clone();
    }
    if v.server.fit.is_some() {
        s.fit = v.server.fit.clone();
    }
    if v.server.fit_target.is_some() {
        s.fit_target = v.server.fit_target.clone();
    }
    set!(opt threads);
    set!(opt threads_batch);
    if let Some(x) = v.speculative.kind.clone() {
        // Spegnere la speculazione vuol dire togliere anche le sue leve: il profilo rifiuta
        // `type = none` con un draft_n_max ereditato dalla base, e ha ragione — non avrebbe effetto.
        if x == "none" {
            p.speculative = aethera_lib::profile::Speculative::default();
        }
        p.speculative.kind = x;
    }
    if v.speculative.draft_n_max.is_some() {
        p.speculative.draft_n_max = v.speculative.draft_n_max;
    }
    if v.speculative.draft_n_min.is_some() {
        p.speculative.draft_n_min = v.speculative.draft_n_min;
    }
    if v.speculative.draft_p_min.is_some() {
        p.speculative.draft_p_min = v.speculative.draft_p_min;
    }
    if v.cache.cache_reuse.is_some() {
        p.cache.cache_reuse = v.cache.cache_reuse;
    }
    if v.cache.ctx_checkpoints.is_some() {
        p.cache.ctx_checkpoints = v.cache.ctx_checkpoints;
    }
    if let Some(b) = &v.build {
        p.runtime.build = b.clone();
    }
    p.server.extra_args.extend(v.extra_args.iter().cloned());
}

fn load_profile(root: &DataRoot, name: &str) -> Result<Profile, String> {
    let path = root.profiles().join(format!("{name}.toml"));
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    aethera_lib::profile::load(&text, name).0.ok_or_else(|| format!("{}: profilo illeggibile", path.display()))
}

fn start_variant(root: &DataRoot, engine: &Engine, sc: &Scenario, v: &Variant) -> Result<Started, String> {
    let machine = root.load_machine()?;
    let report = system::probe().report();
    let base_name = v.profile.clone().unwrap_or_else(|| sc.base.clone());
    let mut p = load_profile(root, &base_name)?;
    patch(&mut p, v);

    let run_id = chrono::Local::now().format("r-%Y%m%d-%H%M%S").to_string();
    let run_dir = root.runs().join(&run_id);
    let prep = launch::prepare(root, &machine, &base_name, &p, &run_dir.join("slots"));
    if !prep.blockers.is_empty() {
        return Err(prep.blockers.join("\n"));
    }
    let run = engine.start(StartRequest {
        run_id: run_id.clone(),
        run_dir: run_dir.clone(),
        base: base_name.clone(),
        profile: p.clone(),
        profile_file: prep.base_file,
        overrides: prep.overrides,
        invalidates_cache: prep.invalidates_cache,
        build: prep.build.ok_or("build non risolta")?,
        model_path: prep.model_path.ok_or("pesi non trovati")?,
        model_size: prep.model_size.unwrap_or(0),
        args: prep.args,
        machine_name: machine.name.clone(),
        ram_margin_gib: machine.ram_margin_gib,
        system: report,
        conditions: system::probe().conditions(None),
        reference: None,
        conditions_changed: Vec::new(),
    })?;
    println!("  riga: {}", run.command_line);

    let t0 = Instant::now();
    let load_ms = loop {
        std::thread::sleep(Duration::from_millis(400));
        match engine.status() {
            EngineStatus::Loading { .. } if t0.elapsed() < Duration::from_secs(1800) => continue,
            EngineStatus::Ready { load_ms, ctx_served, .. } => {
                println!("  pronto in {load_ms} ms · n_ctx servito {ctx_served:?}");
                break load_ms;
            }
            other => {
                let _ = engine.stop();
                return Err(format!("non è arrivato a pronto: {other:?}"));
            }
        }
    };
    Ok(Started {
        run_id,
        base_url: run.base_url,
        manifest_path: PathBuf::from(&run.manifest_path),
        load_ms,
    })
}

/// Aethera rifiuta l'arresto per trenta secondi dopo l'ultima richiesta («motore in uso»): è la
/// protezione che serve quando c'è un client vero. Qui il client siamo noi e abbiamo appena finito,
/// quindi si aspetta che la finestra passi invece di forzare la mano al motore.
fn stop_when_free(engine: &Engine) -> Result<(), String> {
    let t0 = Instant::now();
    loop {
        match engine.stop() {
            Ok(()) => return Ok(()),
            Err(e) if e.contains("in uso") && t0.elapsed() < Duration::from_secs(90) => {
                std::thread::sleep(Duration::from_secs(3));
            }
            Err(e) => return Err(e),
        }
    }
}

// ---------------------------------------------------------------- main

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = DataRoot::new(args.first().ok_or("uso: m08_bench <radice> <scenario.toml> [--dry]")?);
    let scenario_path = PathBuf::from(args.get(1).ok_or("scenario?")?);
    let dry = args.iter().any(|a| a == "--dry");
    let sc: Scenario = toml::from_str(&std::fs::read_to_string(&scenario_path).map_err(|e| format!("{}: {e}", scenario_path.display()))?)
        .map_err(|e| format!("{}: {e}", scenario_path.display()))?;

    let dir = scenario_path.parent().unwrap_or(Path::new("."));
    // I risultati vanno nella cartella che ha il nome di quella dello scenario (m08, m10…).
    let out_dir = root.path.join(dir.file_name().and_then(|n| n.to_str()).unwrap_or("m08"));
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    let out_path = out_dir.join(format!("{}.jsonl", sc.measure));
    let meta_path = out_dir.join(format!("{}.meta.json", sc.measure));

    println!("=== {} · {} ===", sc.measure, sc.title);
    println!("profilo base: {} · {} varianti · {} carichi · {} giri", sc.base, sc.variant.len(), sc.workload.len(), sc.repetitions);

    // I prompt si leggono una volta sola: sono congelati, e restano gli stessi per tutte le varianti.
    let mut bodies = Vec::new();
    for w in &sc.workload {
        let p = dir.join(&w.prompt);
        let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        println!("carico «{}»: {} · {} caratteri", w.name, w.prompt, text.len());
        bodies.push(text);
    }
    if dry {
        println!("(--dry: nessun avvio)");
        return Ok(());
    }

    let engine = Engine::default();
    let mut out = std::fs::OpenOptions::new().create(true).append(true).open(&out_path).map_err(|e| e.to_string())?;
    let mut meta = Vec::new();

    for v in &sc.variant {
        println!("\n--- variante «{}» {} ---", v.name, v.note.clone().unwrap_or_default());
        let started = match start_variant(&root, &engine, &sc, v) {
            Ok(s) => s,
            Err(e) => {
                println!("  NON AVVIATA: {e}");
                meta.push(json!({"variant": v.name, "error": e}));
                let _ = engine.stop();
                continue;
            }
        };
        for (wi, w) in sc.workload.iter().enumerate() {
            let body = &bodies[wi];
            let turns = if w.kind == "turns" { w.turns } else { 1 };
            let total = sc.warmup + sc.repetitions;
            let mut decode = Vec::new();
            let mut prefill = Vec::new();
            for rep in 0..total {
                let warm = rep < sc.warmup;
                let nonce = format!(
                    "{} giro {rep} · {}",
                    v.name,
                    chrono::Local::now().timestamp_nanos_opt().unwrap_or(0)
                );
                for turn in 0..turns {
                    let prompt = turn_prompt(w, body, &nonce, turn);
                    let row = one_request(
                        &started.base_url,
                        w,
                        &prompt,
                        &sc.measure,
                        &v.name,
                        &started.run_id,
                        turn,
                        rep,
                        warm,
                    )?;
                    let pps = row.timings["prompt_per_second"].as_f64();
                    let dps = row.timings["predicted_per_second"].as_f64();
                    println!(
                        "  {} {}{}.{} · prefill {:>7.1} tok/s ({:?} tok) · decode {:>5.2} tok/s · cache_n {:?} · slots {:?} · {} ms",
                        w.name,
                        if warm { "riscaldamento " } else { "" },
                        rep,
                        turn,
                        pps.unwrap_or(f64::NAN),
                        row.timings["prompt_n"].as_u64(),
                        dps.unwrap_or(f64::NAN),
                        row.timings["cache_n"].as_u64(),
                        row.cache_tokens,
                        row.wall_ms
                    );
                    if !warm {
                        if let Some(x) = dps {
                            decode.push(x);
                        }
                        if let Some(x) = pps {
                            prefill.push(x);
                        }
                    }
                    writeln!(out, "{}", serde_json::to_string(&row).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
                    out.flush().map_err(|e| e.to_string())?;
                }
            }
            println!(
                "  → «{}» mediane: prefill {:?} tok/s (sd {:?}) · decode {:?} tok/s (sd {:?})",
                w.name,
                med(&prefill).map(|x| (x * 10.0).round() / 10.0),
                spread(&prefill).map(|x| (x * 10.0).round() / 10.0),
                med(&decode).map(|x| (x * 100.0).round() / 100.0),
                spread(&decode).map(|x| (x * 100.0).round() / 100.0),
            );
        }
        // Il manifest si rilegge **ora**, non appena il motore è pronto: la memoria misurata ci
        // finisce qualche secondo dopo il «pronto», quando i buffer di calcolo sono allocati.
        let m = manifest::read(&started.manifest_path).ok();
        let mem = m.as_ref().and_then(|m| m.memory.clone()).and_then(|m| m.after_load);
        if let Some(mem) = &mem {
            println!(
                "  memoria: VRAM dedicata {:?} GiB · condivisa {:?} GiB · working set {:?} GiB · RAM libera {:?} GiB · doppia copia {:?}",
                mem.vram_dedicated_gib, mem.vram_shared_gib, mem.working_set_gib, mem.ram_available_gib, mem.double_copy
            );
        }
        meta.push(json!({
            "variant": v.name,
            "note": v.note,
            "run_id": started.run_id,
            "load_ms": started.load_ms,
            "command": m.as_ref().map(|m| m.command.line.clone()),
            "build": m.as_ref().and_then(|m| m.engine.build.clone()),
            "memory_before": m.as_ref().and_then(|m| m.memory.clone()).and_then(|m| m.before),
            "memory_after_load": mem,
            "ctx_served": m.as_ref().and_then(|m| m.server.ctx_served),
        }));
        // Il file delle condizioni si riscrive a ogni variante: se la notte si interrompe a metà,
        // quello che è già stato misurato resta leggibile.
        std::fs::write(&meta_path, serde_json::to_string_pretty(&json!({
            "measure": sc.measure, "title": sc.title, "at": chrono::Local::now().to_rfc3339(),
            "scenario_file": scenario_path.display().to_string(),
            "repetitions": sc.repetitions, "warmup": sc.warmup, "variants": meta,
        })).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;

        stop_when_free(&engine)?;
        // Fra una variante e l'altra: il processo deve davvero lasciare la memoria.
        std::thread::sleep(Duration::from_secs(5));
    }

    std::fs::write(&meta_path, serde_json::to_string_pretty(&json!({
        "measure": sc.measure,
        "title": sc.title,
        "at": chrono::Local::now().to_rfc3339(),
        "scenario_file": scenario_path.display().to_string(),
        "repetitions": sc.repetitions,
        "warmup": sc.warmup,
        "variants": meta,
    })).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    println!("\nrighe in {} · condizioni in {}", out_path.display(), meta_path.display());
    Ok(())
}
