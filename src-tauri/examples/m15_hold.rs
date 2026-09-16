//! M-15: tiene acceso un motore da profilo per la batteria di coding agentico, come `m08_hold`,
//! ma con l'endpoint locale di Aethera (127.0.0.1:8090) acceso: il runner della batteria prende e
//! rilascia il lock per ogni compito e legge la telemetria dell'avvio da lì, come fa un client vero.
//!
//! Uso:
//! `m15_hold <radice> <profilo> [--ctx N] [--extra "a b c"] [--template file.jinja] [--pronto file.json]`
//! `         [--cache-ram MiB] [--ctx-checkpoints N]`
//!
//! Quando il motore è pronto scrive `--pronto` (JSON: run id, base_url, alias, contesto servito,
//! endpoint, build, riga di comando). Si ferma quando compare `<radice>/stop-m15`, aspettando che
//! la finestra «in uso» di Aethera passi; con Ctrl+C il job object chiude il figlio.

use aethera_lib::endpoint;
use aethera_lib::engine::{Engine, EngineStatus, StartRequest};
use aethera_lib::machine::DataRoot;
use aethera_lib::{launch, system};
use serde_json::json;
use std::time::{Duration, Instant};

fn arg_after(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned()
}

fn stop_when_free(engine: &Engine) -> Result<(), String> {
    let t0 = Instant::now();
    loop {
        match engine.stop() {
            Ok(()) => return Ok(()),
            Err(e) if e.contains("in uso") && t0.elapsed() < Duration::from_secs(120) => {
                std::thread::sleep(Duration::from_secs(3));
            }
            Err(e) => return Err(e),
        }
    }
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = DataRoot::new(args.first().ok_or("uso: m15_hold <radice> <profilo> [--ctx N] [--extra \"…\"] [--template f] [--pronto f]")?);
    let name = args.get(1).ok_or("profilo?")?.clone();
    let ctx: Option<u32> = arg_after(&args, "--ctx").and_then(|x| x.parse().ok());
    let extra: Vec<String> = arg_after(&args, "--extra").map(|s| s.split_whitespace().map(str::to_string).collect()).unwrap_or_default();
    let template = arg_after(&args, "--template");
    let ready_file = arg_after(&args, "--pronto");

    let machine = root.load_machine()?;
    let path = root.profiles().join(format!("{name}.toml"));
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut p = aethera_lib::profile::load(&text, &name).0.ok_or("profilo illeggibile")?;
    if let Some(c) = ctx {
        p.server.ctx = c;
        if let Some(cl) = p.client.as_mut() {
            cl.context_window = c.saturating_sub(cl.reserved_output_tokens);
        }
    }
    p.server.extra_args.extend(extra);
    // Leve della cache che lo schema gestisce da sé (in `--extra` sarebbero rifiutate): servono a
    // Flash-Next a VGM 48, dove un'entrata della prompt cache vale 0,5-0,6 GiB e la RAM libera 2-3.
    if let Some(n) = arg_after(&args, "--cache-ram").and_then(|x| x.parse().ok()) {
        p.cache.cache_ram = Some(n);
    }
    if let Some(n) = arg_after(&args, "--ctx-checkpoints").and_then(|x| x.parse().ok()) {
        p.cache.ctx_checkpoints = Some(n);
    }
    if template.is_some() {
        p.server.chat_template_file = template;
    }

    let engine = Engine::default();
    let ep = endpoint::spawn(engine.clone(), endpoint::DEFAULT_ADDR)?.to_string();

    let run_id = chrono::Local::now().format("r-%Y%m%d-%H%M%S").to_string();
    let run_dir = root.runs().join(&run_id);
    let prep = launch::prepare(&root, &machine, &name, &p, &run_dir.join("slots"));
    if !prep.blockers.is_empty() {
        return Err(prep.blockers.join("\n"));
    }
    let run = engine.start(StartRequest {
        run_id: run_id.clone(),
        run_dir: run_dir.clone(),
        base: name.clone(),
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
        system: system::probe().report(),
        conditions: system::probe().conditions(machine.models_dir.as_deref()),
        reference: None,
        conditions_changed: Vec::new(),
    })?;
    println!("{}\n{}", run.run_id, run.command_line);

    let t0 = Instant::now();
    let (load_ms, ctx_served) = loop {
        std::thread::sleep(Duration::from_millis(500));
        match engine.status() {
            EngineStatus::Loading { .. } if t0.elapsed() < Duration::from_secs(1800) => continue,
            EngineStatus::Ready { load_ms, ctx_served, .. } => break (load_ms, ctx_served),
            other => {
                let _ = engine.stop();
                return Err(format!("non è arrivato a pronto: {other:?}"));
            }
        }
    };
    println!("PRONTO in {load_ms} ms · n_ctx {ctx_served:?} · {} · endpoint {ep}", run.base_url);
    let m = engine.manifest().ok_or("manifest assente")?;
    if let Some(f) = &ready_file {
        let doc = json!({
            "run_id": run_id,
            "run_dir": run_dir.display().to_string(),
            "base_url": run.base_url,
            "alias": m.server.alias,
            "ctx_declared": m.server.ctx_declared,
            "ctx_served": ctx_served,
            "load_ms": load_ms,
            "endpoint": ep,
            "build": run.build,
            "command_line": run.command_line,
            "pid": std::process::id(),
            "server_pid": run.pid,
        });
        std::fs::write(f, serde_json::to_string_pretty(&doc).unwrap_or_default()).map_err(|e| format!("{f}: {e}"))?;
    }

    let stop = root.path.join("stop-m15");
    let _ = std::fs::remove_file(&stop);
    println!("acceso: fermalo creando {}", stop.display());
    while !stop.exists() {
        if !engine.is_running() {
            return Err(format!("il motore è uscito da solo: {:?}", engine.status()));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    let _ = std::fs::remove_file(&stop);
    stop_when_free(&engine)?;
    println!("fermato · run {run_id}");
    Ok(())
}
