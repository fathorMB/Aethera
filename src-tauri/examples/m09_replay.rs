//! Rilegge il log di un avvio vero con il parser di M-09 e stampa quello che la pagina Motore
//! mostrerebbe: richieste, riuso ricavato dal log, compattazioni. Con `--json` scrive anche i dati
//! per il banco della finestra (stato del motore, storico, righe per i client).
//!
//! Uso: cargo run --example m09_replay -- <radice dati> <id avvio> [--json <file>]

use aethera_lib::clients::{self, RunFacts};
use aethera_lib::conditions::Conditions;
use aethera_lib::manifest;
use aethera_lib::runs;
use aethera_lib::system;
use aethera_lib::telemetry::{self, LogParser, Record, TurnKind};
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (root, id) = match args.as_slice() {
        [r, i, ..] => (PathBuf::from(r), i.clone()),
        _ => return Err("uso: m09_replay <radice dati> <id avvio> [--json <file>]".into()),
    };
    let json_out = args.iter().position(|a| a == "--json").and_then(|i| args.get(i + 1)).map(PathBuf::from);
    let dir = root.join("runs").join(&id);
    let m = manifest::read(&dir.join("manifest.toml"))?;
    let log = std::fs::read_to_string(dir.join("server.log")).map_err(|e| e.to_string())?;
    let old = telemetry::read(&dir);

    let mut parser = LogParser::default();
    let mut records: Vec<Record> = Vec::new();
    for line in log.lines() {
        if let Some(t) = parser.feed(line) {
            let cache = t.cache_from_log();
            let at = old.iter().find(|r| r.task == t.task).map(|r| r.at.clone()).unwrap_or_default();
            records.push(t.into_record(at, cache, Some("log"), None));
        }
    }
    let ubatch = Some(m.effective_profile.server.ubatch);
    let s = telemetry::summarize(&records, usize::MAX, ubatch);

    println!("{} · {} · -ub {:?} · {} richieste", id, m.run.profile, ubatch, records.len());
    println!("{:>6} {:>8} {:>8} {:>8} {:>8} {:>7}  lettura", "task", "prompt", "cache", "/slots", "rielab", "prefill");
    let mut differ = 0;
    for t in &s.turns {
        let slots = old.iter().find(|r| r.task == t.task).and_then(|r| r.cache_n);
        if let (Some(a), Some(b)) = (t.cache_n, slots) {
            if a.abs_diff(b) > 1 {
                differ += 1;
            }
        }
        println!(
            "{:>6} {:>8} {:>8} {:>8} {:>8} {:>6.1}s  {:?}",
            t.task,
            t.prompt_total.map_or("?".into(), |x| x.to_string()),
            t.cache_n.map_or("?".into(), |x| x.to_string()),
            slots.map_or("-".into(), |x| x.to_string()),
            t.prompt_n,
            t.prompt_ms / 1000.0,
            t.kind
        );
    }
    for c in &s.compactions {
        println!("compattazione: task {:?}, {} token rielaborati, {:.1} s", c.tasks, c.reprocessed, c.cost_ms / 1000.0);
    }
    println!("differenze oltre un token fra log e /slots: {differ}");
    let unknown = s.turns.iter().filter(|t| t.kind == TurnKind::Unknown).count();
    println!("richieste non attribuibili: {unknown}");

    if let Some(out) = json_out {
        let history = runs::list(&root.join("runs"), None);
        let conditions = m.conditions.clone().unwrap_or_else(|| system::probe().conditions(None));
        let mut window = telemetry::summarize(&records, telemetry::RECENT, ubatch);
        window.requests = records.len();
        let base_url = format!("http://{}:{}", m.server.host, m.server.port);
        let fixed = clients::measured_fixed_prompts();
        let snippets = clients::snippets(&RunFacts {
            run_id: &m.run.id,
            base_url: &base_url,
            alias: &m.server.alias,
            ctx_declared: m.server.ctx_declared,
            ctx_served: m.server.ctx_served,
            client: m.effective_profile.client.as_ref(),
            endpoint: Some("127.0.0.1:8090"),
            sampling: &m.effective_profile.sampling_by_mode,
            chat_template: Some("qwen3.6-tollerante.jinja"),
            claude_config_dir: Some(root.join("clients").join("claude-code").display().to_string()),
            fixed_prompts: &fixed,
        });
        let v = serde_json::json!({
            "manifest": m,
            "summary": window,
            "conditions": conditions,
            "conditions_changed": ["driver GPU 32.0.22042.1 → 32.0.31041.1004"],
            "reference": { "decode_median": 30.9, "runs": 3 },
            "runs": history,
            "snippets": snippets,
            "overview_conditions": system::probe().conditions(std::env::var_os("AETHERA_PESI").map(PathBuf::from).as_deref()),
        });
        std::fs::write(&out, serde_json::to_string_pretty(&v).unwrap()).map_err(|e| e.to_string())?;
        println!("dati del banco in {}", out.display());
    }
    let _ = Conditions::default();
    Ok(())
}
