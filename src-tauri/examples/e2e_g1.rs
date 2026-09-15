//! Prova end-to-end di M-02 T-10: dal JSON di minis-config a un llama-server acceso da Aethera.
//!
//! Uso: `cargo run --example e2e_g1 -- <radice-dati> <cartella-pesi> <cartella-build>`
//!
//! Importa il profilo G1, prepara l'avvio con lo stesso codice della finestra, avvia il motore in
//! job object e aspetta «pronto». Resta acceso finché non compare `<radice>/stop-e2e` (così un
//! client esterno può fare il suo preflight), poi ferma il motore e stampa il manifest.

use aethera_lib::engine::{Engine, EngineStatus, StartRequest};
use aethera_lib::machine::{BuildEntry, DataRoot, MachineConfig};
use aethera_lib::{import, launch, system};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const G1_JSON: &str = r"C:\Git\minis-config\profiles\qwen3.6-35b-a3b.q4_k_m.vulkan.json";

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [root, models, build_dir] = args.as_slice() else {
        return Err("uso: e2e_g1 <radice-dati> <cartella-pesi> <cartella-build>".into());
    };

    let report = system::probe().report();
    let root = DataRoot::new(root);
    let mut machine = root.ensure(&report)?;
    machine = MachineConfig {
        models_dir: Some(PathBuf::from(models)),
        builds: vec![BuildEntry { id: "b10809-vulkan".into(), path: PathBuf::from(build_dir) }],
        ..machine
    };
    root.save_machine(&machine)?;

    let text = std::fs::read_to_string(G1_JSON).map_err(|e| format!("{G1_JSON}: {e}"))?;
    let imported = import::import_minis_json(&text, "qwen3.6-35b-a3b.q4_k_m.vulkan.json", "b10809")?;
    let profile = imported.profile;
    let profile_file = root.profiles().join(format!("{}.toml", profile.name));
    std::fs::write(&profile_file, toml::to_string_pretty(&profile).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    println!("profilo: {}", profile_file.display());

    let run_id = chrono::Local::now().format("r-%Y%m%d-%H%M%S").to_string();
    let run_dir = root.runs().join(&run_id);
    let p = launch::prepare(&root, &machine, &profile.name, &profile, &run_dir.join("slots"));
    if !p.blockers.is_empty() {
        return Err(p.blockers.join("\n"));
    }
    println!("differenze dal profilo: {}", p.overrides.len());

    let engine = Engine::default();
    let run = engine.start(StartRequest {
        run_id,
        run_dir,
        profile: profile.clone(),
        profile_file: p.base_file,
        overrides: p.overrides,
        invalidates_cache: p.invalidates_cache,
        build: p.build.expect("build"),
        model_path: p.model_path.expect("pesi"),
        model_size: p.model_size.unwrap_or(0),
        args: p.args,
        machine_name: machine.name.clone(),
        system: report,
    })?;
    println!("avviato {} pid {}\n{}", run.run_id, run.pid, run.command_line);

    let t0 = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(500));
        match engine.status() {
            EngineStatus::Loading { .. } if t0.elapsed() < Duration::from_secs(600) => continue,
            EngineStatus::Ready { load_ms, ctx_served, alias_served, divergences, .. } => {
                println!(
                    "PRONTO in {load_ms} ms · n_ctx servito {ctx_served:?} · alias {alias_served:?} · divergenze {divergences:?}"
                );
                break;
            }
            other => {
                let _ = engine.stop();
                return Err(format!("il motore non è arrivato a pronto: {other:?}"));
            }
        }
    }

    let stop_file = root.path.join("stop-e2e");
    println!("in attesa di {} …", stop_file.display());
    while !stop_file.exists() {
        if !engine.is_running() {
            return Err(format!("il motore è uscito da solo: {:?}", engine.status()));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    let _ = std::fs::remove_file(&stop_file);
    engine.stop()?;
    println!("fermato: {:?}", engine.status());
    println!("--- manifest ---\n{}", std::fs::read_to_string(&run.manifest_path).map_err(|e| e.to_string())?);
    Ok(())
}
