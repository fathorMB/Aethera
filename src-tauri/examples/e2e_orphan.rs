//! Prova di M-03 T-07 sulla macchina vera: un `llama-server` avviato fuori da Aethera sulla porta di
//! un profilo è riconosciuto come orfano (sola lettura) e si termina su richiesta.
//!
//! Uso: `cargo run --example e2e_orphan -- <radice-dati> [profilo]`

use aethera_lib::endpoint;
use aethera_lib::engine::{Engine, EngineStatus};
use aethera_lib::machine::DataRoot;
use aethera_lib::{launch, profile};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = DataRoot::new(args.first().ok_or("uso: e2e_orphan <radice-dati> [profilo]")?);
    let name = args.get(1).map(String::as_str).unwrap_or("qwen3.6-35b-a3b.q4_k_m.vulkan");
    let machine = root.load_machine()?;
    let text = std::fs::read_to_string(root.profiles().join(format!("{name}.toml"))).map_err(|e| e.to_string())?;
    let p = profile::load(&text, name).0.ok_or("profilo illeggibile")?;
    let slots = std::env::temp_dir().join("aethera-orphan-slots");
    let prep = launch::prepare(&root, &machine, name, &p, &slots);
    let build = prep.build.ok_or("build non trovata")?;

    // Un llama-server lanciato «a mano», fuori dal job object di Aethera.
    std::fs::create_dir_all(&slots).map_err(|e| e.to_string())?;
    let log_path = std::env::temp_dir().join("aethera-orphan-server.log");
    let log = std::fs::File::create(&log_path).map_err(|e| e.to_string())?;
    let mut child = Command::new(&build.binary)
        .args(&prep.args)
        .current_dir(&build.dir)
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(|e| e.to_string())?)
        .stderr(log)
        .spawn()
        .map_err(|e| e.to_string())?;
    println!("llama-server esterno pid {} · log {}", child.id(), log_path.display());
    let base = format!("http://{}:{}", p.server.host, p.server.port);
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(2)))
        .http_status_as_error(false)
        .build()
        .into();
    let t0 = Instant::now();
    while agent.get(&format!("{base}/health")).call().map(|r| r.status().as_u16()).ok() != Some(200) {
        if let Ok(Some(code)) = child.try_wait() {
            return Err(format!("il server esterno è uscito: {code:?}, vedi {}", log_path.display()));
        }
        if t0.elapsed() > Duration::from_secs(120) {
            let _ = child.kill();
            return Err(format!("il server esterno non è pronto entro 120 s, vedi {}", log_path.display()));
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    let engine = Engine::default();
    let ep = endpoint::spawn(engine.clone(), "127.0.0.1:0")?;
    engine.scan_orphans(&[(p.server.host.clone(), p.server.port)]);
    let status = engine.status();
    println!("stato: {status:?}");
    let (s, v) = endpoint::route(&engine, "GET", "/status", false, b"");
    println!("endpoint {ep} /status → {s} {v}");
    let ok = match &status {
        EngineStatus::Orphan { orphan, .. } => orphan.pid == child.id() && orphan.alias.as_deref() == Some(name),
        _ => false,
    };
    if !ok {
        let _ = child.kill();
        return Err("orfano non riconosciuto".into());
    }
    println!("✓ orfano riconosciuto con PID e alias");
    let refused = engine.terminate_orphan(child.id() + 1);
    println!("terminate con un PID diverso → {refused:?}");
    engine.terminate_orphan(child.id())?;
    let code = child.wait().map_err(|e| e.to_string())?;
    println!("✓ orfano terminato su richiesta: {code:?}");
    engine.scan_orphans(&[(p.server.host.clone(), p.server.port)]);
    if !matches!(engine.status(), EngineStatus::Off { .. }) {
        return Err(format!("dopo la terminazione lo stato non è spento: {:?}", engine.status()));
    }
    println!("✓ stato tornato spento\nE2E orfano: tutto verde");
    Ok(())
}
