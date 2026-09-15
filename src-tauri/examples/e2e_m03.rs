//! Prova di M-03 T-10 sulla macchina vera: un profilo acceso da Aethera, un client reale che manda
//! richieste con un prefisso lungo in comune, memoria misurata, quota di cache, lock dall'endpoint
//! che rifiuta l'arresto.
//!
//! Uso: `cargo run --example e2e_m03 -- <radice-dati> [profilo]`
//! La radice deve avere `machine.toml` con pesi e build e il profilo in `profiles/` (quelli di M-02).

use aethera_lib::endpoint;
use aethera_lib::engine::{Engine, EngineStatus, StartRequest};
use aethera_lib::machine::DataRoot;
use aethera_lib::{launch, profile, system, telemetry};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

const DEFAULT_PROFILE: &str = "qwen3.6-35b-a3b.q4_k_m.vulkan";

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(600)))
        .http_status_as_error(false)
        .build()
        .into()
}

fn call(method: &str, url: &str, body: Option<Value>) -> Result<(u16, Value), String> {
    let a = agent();
    let mut resp = match (method, body) {
        ("GET", _) => a.get(url).call(),
        ("DELETE", _) => a.delete(url).call(),
        (_, Some(b)) => a.post(url).send_json(&b),
        (_, None) => a.post(url).send_empty(),
    }
    .map_err(|e| format!("{method} {url}: {e}"))?;
    let status = resp.status().as_u16();
    let v = resp.body_mut().read_json::<Value>().unwrap_or(Value::Null);
    Ok((status, v))
}

fn check(ok: bool, what: &str, failures: &mut Vec<String>) {
    println!("{} {what}", if ok { "✓" } else { "✗" });
    if !ok {
        failures.push(what.to_string());
    }
}

/// Codice vero come prefisso: una frase ripetuta gonfierebbe accettazione e cache senza dire niente.
fn code_prefix() -> String {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/engine.rs")).unwrap_or_default();
    format!("Sei un revisore di codice Rust. Questo è il file engine.rs di un launcher:\n\n```rust\n{src}\n```")
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = DataRoot::new(args.first().ok_or("uso: e2e_m03 <radice-dati> [profilo]")?);
    let name = args.get(1).map(String::as_str).unwrap_or(DEFAULT_PROFILE);
    let machine = root.load_machine()?;
    let text = std::fs::read_to_string(root.profiles().join(format!("{name}.toml"))).map_err(|e| e.to_string())?;
    let (p, issues) = profile::load(&text, name);
    let p = p.ok_or_else(|| format!("{issues:?}"))?;
    let mut failures = Vec::new();

    let engine = Engine::default();
    let ep = endpoint::spawn(engine.clone(), endpoint::DEFAULT_ADDR)?;
    let ep_url = format!("http://{ep}");
    println!("endpoint {ep_url}");

    let run_id = chrono::Local::now().format("r-%Y%m%d-%H%M%S").to_string();
    let run_dir = root.runs().join(&run_id);
    let prep = launch::prepare(&root, &machine, name, &p, &run_dir.join("slots"));
    if !prep.blockers.is_empty() {
        return Err(prep.blockers.join("\n"));
    }
    let run = engine.start(StartRequest {
        run_id,
        run_dir: run_dir.clone(),
        base: name.to_string(),
        profile: p.clone(),
        profile_file: prep.base_file,
        overrides: prep.overrides,
        invalidates_cache: prep.invalidates_cache,
        build: prep.build.expect("build"),
        model_path: prep.model_path.expect("pesi"),
        model_size: prep.model_size.unwrap_or(0),
        args: prep.args,
        machine_name: machine.name.clone(),
        ram_margin_gib: machine.ram_margin_gib,
        system: system::probe().report(),
    })?;
    println!("avviato {} pid {}", run.run_id, run.pid);

    let t0 = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(500));
        match engine.status() {
            EngineStatus::Loading { .. } if t0.elapsed() < Duration::from_secs(600) => continue,
            EngineStatus::Ready { load_ms, ctx_served, .. } => {
                println!("PRONTO in {load_ms} ms · n_ctx servito {ctx_served:?}");
                break;
            }
            other => return Err(format!("il motore non è arrivato a pronto: {other:?}")),
        }
    }

    // Memoria dopo il caricamento.
    let t1 = Instant::now();
    let memory = loop {
        if let Some(m) = engine.manifest().and_then(|m| m.memory).filter(|m| m.after_load.is_some()) {
            break m;
        }
        if t1.elapsed() > Duration::from_secs(20) {
            return Err("memoria dopo il caricamento non letta entro 20 s".into());
        }
        std::thread::sleep(Duration::from_millis(250));
    };
    let before = memory.before.clone().unwrap_or_default();
    let after = memory.after_load.clone().unwrap();
    println!("memoria prima: {before:?}");
    println!("memoria dopo:  {after:?}");
    println!("riferimento serve.ps1 (15-09): VRAM dedicata 22,68 GB · RAM disponibile 33,66 GB");
    check(before.vram_free_mib.is_some(), "VRAM libera prima dell'avvio da --list-devices", &mut failures);
    check(after.vram_dedicated_gib.is_some_and(|v| (v - 22.68).abs() < 1.0), "VRAM dedicata del processo entro 1 GiB da 22,68", &mut failures);
    check(after.vram_shared_gib.is_some() && after.working_set_gib.is_some(), "VRAM condivisa e working set letti", &mut failures);
    check(after.double_copy == Some(false), "nessuna doppia copia dei pesi con load_mode auto", &mut failures);
    check(after.margin_ok.is_some(), "soglia di margine RAM valutata", &mut failures);

    // Il client si dichiara.
    let (s, v) = call("POST", &format!("{ep_url}/lock"), Some(json!({ "client": "e2e-m03", "label": "prova T-10", "ttl_s": 600 })))?;
    println!("POST /lock → {s} {v}");
    check(s == 201 && v["run_id"] == run.run_id, "POST /lock restituisce l'id dell'avvio", &mut failures);
    let lock_id = v["lock"]["id"].as_str().unwrap_or_default().to_string();
    let stop = engine.stop();
    println!("stop con il lock → {stop:?}");
    check(stop.as_ref().is_err_and(|e| e.contains("lock e2e-m03")), "il lock rifiuta l'arresto", &mut failures);

    // Tre richieste con lo stesso prefisso lungo: la seconda e la terza devono uscire dalla cache.
    let prefix = code_prefix();
    let questions = ["Riassumi in una frase cosa fa monitor().", "Quale costante regola la finestra dello stato in uso?", "Che cosa fa detach()?"];
    for q in questions {
        let t = Instant::now();
        let (s, v) = call(
            "POST",
            &format!("{}/v1/chat/completions", run.base_url),
            Some(json!({
                "model": p.name,
                "messages": [{ "role": "system", "content": prefix }, { "role": "user", "content": q }],
                "max_tokens": 64,
                "temperature": 0.7,
                "chat_template_kwargs": { "enable_thinking": false },
            })),
        )?;
        println!("richiesta «{q}» → {s} in {:.1} s · timings {}", t.elapsed().as_secs_f64(), v["timings"]);
    }

    let t2 = Instant::now();
    while engine.recent(10).map(|r| r.2.len()).unwrap_or(0) < questions.len() {
        if t2.elapsed() > Duration::from_secs(15) {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let (_, summary, records) = engine.recent(10).unwrap_or_default();
    for r in &records {
        println!("telemetria: {}", serde_json::to_string(r).unwrap());
    }
    println!("riepilogo: {}", serde_json::to_string(&summary).unwrap());
    check(records.len() == questions.len(), "una riga di telemetria per richiesta", &mut failures);
    check(records.first().is_some_and(|r| r.cache_n == Some(0)), "prima richiesta: 0 token dalla cache, non sconosciuto", &mut failures);
    check(records.iter().skip(1).all(|r| r.cache_n.is_some_and(|c| c > 1000)), "richieste 2 e 3 con token dalla cache attribuiti", &mut failures);
    check(summary.cache_share.is_some_and(|c| c > 0.5), "quota cache visibile e sopra il 50 %", &mut failures);
    check(summary.decode_median.is_some(), "decode mediano misurato", &mut failures);
    check(telemetry::read(&run_dir).len() == questions.len(), "telemetry.jsonl scritto su disco", &mut failures);

    let (s, v) = call("GET", &format!("{ep_url}/status"), None)?;
    println!("GET /status → {s} {v}");
    check(s == 200 && v["in_use"] == true && v["run_id"] == run.run_id, "GET /status dice in uso con l'id dell'avvio", &mut failures);
    let (s, v) = call("GET", &format!("{ep_url}/telemetry/recent?n=5"), None)?;
    check(s == 200 && v["records"].as_array().is_some_and(|a| a.len() == questions.len()), "GET /telemetry/recent", &mut failures);
    let (s, v) = call("GET", &format!("{ep_url}/run"), None)?;
    check(s == 200 && v["run"]["id"] == run.run_id, "GET /run restituisce il manifest", &mut failures);

    let (s, v) = call("DELETE", &format!("{ep_url}/lock?id={lock_id}"), None)?;
    println!("DELETE /lock → {s} {v}");
    check(s == 200 && v["released"] == 1, "DELETE /lock rilascia", &mut failures);
    let stop = engine.stop();
    println!("stop subito dopo le richieste → {stop:?}");
    check(stop.as_ref().is_err_and(|e| e.contains("richiesta")), "richieste degli ultimi 30 s rifiutano l'arresto", &mut failures);

    println!("attendo che il motore torni libero…");
    let t3 = Instant::now();
    while engine.usage().in_use && t3.elapsed() < Duration::from_secs(45) {
        std::thread::sleep(Duration::from_millis(500));
    }
    let stop = engine.stop();
    check(stop.is_ok(), "arresto accettato a motore libero", &mut failures);
    println!("--- manifest ---\n{}", std::fs::read_to_string(&run.manifest_path).map_err(|e| e.to_string())?);

    if failures.is_empty() {
        println!("E2E M-03: tutto verde");
        Ok(())
    } else {
        Err(format!("E2E M-03: {} controlli falliti: {failures:?}", failures.len()))
    }
}
