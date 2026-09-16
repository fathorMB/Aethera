//! Prova di M-09 T-10 sulla macchina vera: il G1 acceso da Aethera con le proposte di M-08 e il
//! template tollerante, poi Nonio, OpenCode e Claude Code che ci lavorano con le righe che Aethera
//! dà per ciascuno. Controlla che le condizioni finiscano nel manifest, che il riuso si legga dal
//! log, che ogni client sia riconosciuto dal suo lock e che Claude Code non riceva errori 500.
//!
//! Uso: `cargo run --example e2e_m09 -- <radice-dati> <cartella di lavoro> [client…]`
//! I client sono `nonio`, `opencode`, `claude`; senza, tutti e tre. La cartella di lavoro viene
//! riempita con una copia di pochi sorgenti di Aethera: i client la leggono e basta.

use aethera_lib::clients::{self, RunFacts};
use aethera_lib::endpoint;
use aethera_lib::engine::{Engine, EngineStatus, StartRequest};
use aethera_lib::machine::DataRoot;
use aethera_lib::telemetry::TurnKind;
use aethera_lib::{launch, profile, proposals, runs, system};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const G1: &str = "qwen3.6-35b-a3b.q4_k_m.vulkan";
/// Eseguibile di Nonio: `AETHERA_NONIO_EXE`, altrimenti `nonio.exe` dal PATH.
fn nonio() -> String {
    std::env::var("AETHERA_NONIO_EXE").unwrap_or_else(|_| "nonio.exe".into())
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(30))).http_status_as_error(false).build().into()
}

fn lock(ep: &str, client: &str) -> Result<(), String> {
    let r = agent()
        .post(&format!("http://{ep}/lock"))
        .send_json(&json!({ "client": client, "label": "e2e_m09", "ttl_s": 1800 }))
        .map_err(|e| e.to_string())?;
    r.status().is_success().then_some(()).ok_or_else(|| format!("lock {client}: HTTP {}", r.status()))
}

fn unlock(ep: &str, client: &str) {
    let _ = agent().delete(&format!("http://{ep}/lock?client={client}")).call();
}

fn check(ok: bool, what: &str, failures: &mut Vec<String>) {
    println!("{} {what}", if ok { "✓" } else { "✗" });
    if !ok {
        failures.push(what.to_string());
    }
}

/// Esegue un client e ne stampa la coda dell'output; `Err` solo se non è partito.
fn run_client(label: &str, mut cmd: Command) -> Result<(bool, String), String> {
    println!("\n--- {label} ---");
    let t = Instant::now();
    let out = cmd.output().map_err(|e| format!("{label}: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    let tail: Vec<&str> = text.lines().rev().take(12).collect();
    for l in tail.iter().rev() {
        println!("  | {}", l.chars().take(200).collect::<String>());
    }
    println!("  {label}: uscita {:?} in {:.0} s", out.status.code(), t.elapsed().as_secs_f64());
    Ok((out.status.success(), text))
}

/// Variabili d'ambiente dal blocco bash che Aethera dà per Claude Code: la riga copiata è la prova.
fn exports(block: &str) -> Vec<(String, String)> {
    block
        .lines()
        .filter_map(|l| l.strip_prefix("export "))
        .filter_map(|l| {
            let (k, rest) = l.split_once('=')?;
            let v = rest.split('"').nth(1)?;
            Some((k.to_string(), v.to_string()))
        })
        .collect()
}

fn prepare_workspace(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    for f in ["conditions.rs", "proposals.rs", "clients.rs"] {
        std::fs::copy(src.join(f), dir.join(f)).map_err(|e| format!("{f}: {e}"))?;
    }
    // Nonio chiude un compito con il diff del repository (RF-21): senza git esce con 2.
    if !dir.join(".git").exists() {
        for args in [
            vec!["init", "-q"],
            vec!["add", "conditions.rs", "proposals.rs", "clients.rs"],
            vec!["-c", "user.name=e2e_m09", "-c", "user.email=e2e@localhost", "commit", "-q", "-m", "copia per la prova"],
        ] {
            Command::new("git").args(&args).current_dir(dir).status().map_err(|e| format!("git: {e}"))?;
        }
    }
    Ok(())
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = DataRoot::new(args.first().ok_or("uso: e2e_m09 <radice-dati> <cartella di lavoro> [client…]")?);
    let ws = PathBuf::from(args.get(1).ok_or("manca la cartella di lavoro")?);
    let wanted: Vec<&str> = if args.len() > 2 { args[2..].iter().map(String::as_str).collect() } else { vec!["nonio", "opencode", "claude"] };
    let mut failures = Vec::new();
    prepare_workspace(&ws)?;

    let machine = root.ensure(&system::probe().report())?;
    let template = root.templates().join("qwen3.6-tollerante.jinja");
    check(template.is_file(), "il template tollerante è nella radice dati", &mut failures);

    // Il G1 come lo lascerebbe la pagina Avvio dopo «Applica come modifiche», più contesto e template.
    let text = std::fs::read_to_string(root.profiles().join(format!("{G1}.toml"))).map_err(|e| e.to_string())?;
    let base = profile::load(&text, G1).0.ok_or("G1 illeggibile")?;
    let mut p = base.clone();
    let builds = root.builds_available(&machine);
    let props = proposals::for_profile(&p, &builds);
    println!("proposte: {:?}", props.iter().map(|x| format!("{} → {}", x.field, x.value)).collect::<Vec<_>>());
    for x in &props {
        if x.field == "runtime.build" {
            p.runtime.build = x.value.as_str().unwrap_or_default().to_string();
        }
    }
    p.server.ctx = 65536;
    p.server.chat_template_file = Some("qwen3.6-tollerante.jinja".into());
    if let Some(c) = p.client.as_mut() {
        c.context_window = 65536 - c.reserved_output_tokens;
    }

    let engine = Engine::default();
    let ep = endpoint::spawn(engine.clone(), endpoint::DEFAULT_ADDR)?.to_string();
    let run_id = chrono::Local::now().format("r-%Y%m%d-%H%M%S").to_string();
    let run_dir = root.runs().join(&run_id);
    let prep = launch::prepare(&root, &machine, G1, &p, &run_dir.join("slots"));
    if !prep.blockers.is_empty() {
        return Err(prep.blockers.join("\n"));
    }
    let conditions = system::probe().conditions(machine.models_dir.as_deref());
    let history = runs::list(&root.runs(), None);
    let run = engine.start(StartRequest {
        run_id: run_id.clone(),
        run_dir: run_dir.clone(),
        base: G1.into(),
        profile: p.clone(),
        profile_file: prep.base_file,
        overrides: prep.overrides.clone(),
        invalidates_cache: prep.invalidates_cache,
        build: prep.build.expect("build"),
        model_path: prep.model_path.expect("pesi"),
        model_size: prep.model_size.unwrap_or(0),
        args: prep.args,
        machine_name: machine.name.clone(),
        ram_margin_gib: machine.ram_margin_gib,
        system: system::probe().report(),
        reference: runs::reference(&history, &machine.name, &p.name, &p.runtime.build, &conditions),
        conditions_changed: runs::changed_since(&history, &machine.name, &conditions),
        conditions,
    })?;
    println!("avviato {} · {}", run.run_id, run.command_line);
    check(run.command_line.contains("--chat-template-file"), "la riga di comando passa il template", &mut failures);

    let t0 = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(500));
        match engine.status() {
            EngineStatus::Loading { .. } if t0.elapsed() < Duration::from_secs(600) => continue,
            EngineStatus::Ready { load_ms, ctx_served, .. } => {
                println!("PRONTO in {load_ms} ms · contesto servito {ctx_served:?}");
                break;
            }
            other => return Err(format!("il motore non è arrivato a pronto: {other:?}")),
        }
    }

    let m = engine.manifest().ok_or("manifest assente")?;
    let c = m.conditions.clone().unwrap_or_default();
    println!("condizioni: {} · {:?} · {:?}", c.short(), c.adrenalin, c.weights_disk);
    check(c.gpus.first().and_then(|g| g.version.clone()).is_some(), "il manifest ha la versione del driver GPU", &mut failures);
    check(c.power_overlay.is_some() && c.weights_volume.is_some(), "e alimentazione e volume dei pesi", &mut failures);
    let on_disk = std::fs::read_to_string(run_dir.join("manifest.toml")).unwrap_or_default();
    check(on_disk.contains("[conditions]") || on_disk.contains("[[conditions.gpus]]"), "le condizioni sono scritte in manifest.toml", &mut failures);

    let base_url = format!("http://{}:{}", m.server.host, m.server.port);
    let fixed = clients::measured_fixed_prompts();
    let sn = clients::snippets(&RunFacts {
        run_id: &m.run.id,
        base_url: &base_url,
        alias: &m.server.alias,
        ctx_declared: m.server.ctx_declared,
        ctx_served: m.server.ctx_served,
        client: m.effective_profile.client.as_ref(),
        endpoint: Some(&ep),
        sampling: &m.effective_profile.sampling_by_mode,
        chat_template: m.effective_profile.server.chat_template_file.as_deref(),
        claude_config_dir: Some(ws.join(".claude-config").display().to_string()),
        fixed_prompts: &fixed,
    });
    check(sn.claude_code_ready, "le righe dicono che Claude Code è pronto", &mut failures);

    let task = "Leggi conditions.rs e rispondi in tre righe: che cosa fa la funzione changes_since e perché una condizione sconosciuta non conta come cambiata? Non modificare nessun file.";
    let mut done: Vec<(&str, usize)> = Vec::new();

    for client in &wanted {
        let before = engine.recent(usize::MAX).map(|(_, s, _)| s.requests).unwrap_or(0);
        lock(&ep, client)?;
        let outcome = match *client {
            "nonio" => {
                let prof = ws.join("nonio-aethera.toml");
                let toml = format!(
                    "name = \"aethera-e2e\"\n{}\n\n[family]\nkind = \"qwen\"\nthinking = false\n\n[context]\ndeclared = {}\n\n[sampling]\ntemperature = 0.7\ntop_p = 0.8\ntop_k = 20\nmin_p = 0.0\npresence_penalty = 1.5\nmax_tokens = 4096\n",
                    // La riga di Aethera così com'è, più il timeout; il campionamento lo dà la sezione sotto.
                    sn.toml.split("\n\n# Campionamento").next().unwrap_or_default().replace("[backend]\n", "[backend]\ntimeout_s = 600\n"),
                    m.server.ctx_served.unwrap_or(65536)
                );
                std::fs::write(&prof, toml).map_err(|e| e.to_string())?;
                let mut cmd = Command::new(nonio());
                cmd.args(["run", "--max-turns", "6", "--budget-wallclock", "600", "-p"]).arg(&prof).arg("-w").arg(&ws).arg(task);
                run_client("Nonio", cmd)?
            }
            "opencode" => {
                std::fs::write(ws.join("opencode.json"), &sn.opencode).map_err(|e| e.to_string())?;
                // Il modello si forza: se il provider di Aethera non si carica, OpenCode deve fermarsi
                // con un errore invece di ricadere sul suo modello in cloud (è successo al primo giro).
                let model = format!("aethera/{}", m.server.alias);
                let mut cmd = Command::new("cmd");
                cmd.args(["/C", "opencode", "run", "-m", &model, task]).current_dir(&ws).env("PWD", &ws);
                run_client("OpenCode", cmd)?
            }
            "claude" => {
                let mut cmd = Command::new("cmd");
                cmd.args(["/C", "claude", "-p", task, "--allowedTools=Read,Grep,Glob"]).current_dir(&ws).env("PWD", &ws);
                for (k, v) in exports(&sn.claude_code_bash) {
                    cmd.env(k, v);
                }
                // La riga facoltativa, qui usata: la prova non tocca la configurazione di tutti i giorni.
                cmd.env("CLAUDE_CONFIG_DIR", ws.join(".claude-config"));
                run_client("Claude Code", cmd)?
            }
            other => return Err(format!("client sconosciuto: {other}")),
        };
        // Le ultime richieste vengono registrate al giro successivo del monitor.
        std::thread::sleep(Duration::from_secs(2));
        unlock(&ep, client);
        let after = engine.recent(usize::MAX).map(|(_, s, _)| s.requests).unwrap_or(0);
        // Nonio esce con 2 quando non può verificare il compito (nessun comando di verifica, file
        // nuovi del banco): conta che il modello abbia risposto senza errori d'infrastruttura.
        let finished = match *client {
            "nonio" => outcome.1.contains("— ended: the model answered") && !outcome.1.contains("infrastructure failure"),
            _ => outcome.0,
        };
        check(finished, &format!("{client} finisce il compito"), &mut failures);
        check(after > before, &format!("{client} ha mandato {} richieste al motore", after - before), &mut failures);
        if *client == "claude" {
            check(!outcome.1.contains("500") || !outcome.1.contains("System message"), "Claude Code non riceve errori 500", &mut failures);
        }
        done.push((client, after - before));
    }

    let (_, s, _) = engine.recent(usize::MAX).ok_or("nessun avvio")?;
    println!("\n{:>6} {:>10} {:>8} {:>8} {:>8} {:>7}  lettura", "task", "client", "prompt", "cache", "rielab", "prefill");
    for t in &s.turns {
        println!(
            "{:>6} {:>10} {:>8} {:>8} {:>8} {:>6.1}s  {:?}",
            t.task,
            t.client.clone().unwrap_or_else(|| "-".into()),
            t.prompt_total.map_or("?".into(), |x| x.to_string()),
            t.cache_n.map_or("?".into(), |x| x.to_string()),
            t.prompt_n,
            t.prompt_ms / 1000.0,
            t.kind
        );
    }
    check(s.cache_sources == vec!["log".to_string()], "tutta la cache viene dal log", &mut failures);
    check(s.turns.iter().all(|t| t.kind != TurnKind::Unknown), "ogni richiesta ha una lettura", &mut failures);
    for (client, n) in &done {
        let mine: Vec<_> = s.turns.iter().filter(|t| t.client.as_deref() == Some(*client)).collect();
        check(mine.len() == *n, &format!("le {n} richieste di {client} portano il suo nome"), &mut failures);
        if let Some(f) = clients::fixed_from_turns(&s.turns, &run_id).iter().find(|f| f.client == clients::display_name(client)) {
            println!("prompt fisso di {client}: {} token ({})", f.tokens, f.source);
        }
        let kept: Vec<_> = mine.iter().skip(1).filter(|t| t.kind == TurnKind::Extends).collect();
        println!("{client}: {} richieste dopo la prima, {} estendono", mine.len().saturating_sub(1), kept.len());
    }
    for c in &s.compactions {
        println!("compattazione {:?}: {} token, {:.1} s", c.tasks, c.reprocessed, c.cost_ms / 1000.0);
    }
    let log = std::fs::read_to_string(run_dir.join("server.log")).unwrap_or_default();
    check(!log.contains("System message must be at the beginning"), "il motore non ha rifiutato nessun messaggio di sistema", &mut failures);

    // Il motore resta «in uso» per 30 s dopo l'ultima richiesta: si aspetta, non si forza.
    let t2 = Instant::now();
    while let Err(e) = engine.stop() {
        if t2.elapsed() > Duration::from_secs(90) {
            return Err(e);
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    println!("\nmotore fermato · avvio {run_id}");
    if failures.is_empty() {
        println!("E2E M-09: tutto verde");
        Ok(())
    } else {
        Err(format!("E2E M-09: {} controlli falliti: {failures:?}", failures.len()))
    }
}
