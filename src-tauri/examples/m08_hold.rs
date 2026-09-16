//! M-08: tiene acceso un motore da profilo finché non compare il file di stop, per le misure che
//! hanno bisogno di un motore vivo mentre lavora qualcos'altro — i client di T-10, la NPU di T-11,
//! il congelamento dei prompt con `/tokenize`.
//!
//! Uso: `cargo run --release --example m08_hold -- <radice> <profilo> [--ctx N] [--extra "a b c"]`
//! Si ferma quando compare `<radice>/stop-m08` (o con Ctrl+C: il job object chiude il figlio).

use aethera_lib::engine::{Engine, EngineStatus, StartRequest};
use aethera_lib::machine::DataRoot;
use aethera_lib::{launch, system};
use std::time::{Duration, Instant};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = DataRoot::new(args.first().ok_or("uso: m08_hold <radice> <profilo>")?);
    let name = args.get(1).ok_or("profilo?")?.clone();
    let ctx: Option<u32> = args.iter().position(|a| a == "--ctx").and_then(|i| args.get(i + 1)).and_then(|x| x.parse().ok());
    let extra: Vec<String> = args
        .iter()
        .position(|a| a == "--extra")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.split_whitespace().map(str::to_string).collect())
        .unwrap_or_default();

    let machine = root.load_machine()?;
    let report = system::probe().report();
    let path = root.profiles().join(format!("{name}.toml"));
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut p = aethera_lib::profile::load(&text, &name).0.ok_or("profilo illeggibile")?;
    if let Some(c) = ctx {
        p.server.ctx = c;
    }
    p.server.extra_args.extend(extra);

    let run_id = chrono::Local::now().format("r-%Y%m%d-%H%M%S").to_string();
    let run_dir = root.runs().join(&run_id);
    let prep = launch::prepare(&root, &machine, &name, &p, &run_dir.join("slots"));
    if !prep.blockers.is_empty() {
        return Err(prep.blockers.join("\n"));
    }
    let engine = Engine::default();
    let run = engine.start(StartRequest {
        run_id: run_id.clone(),
        run_dir,
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
        system: report,
    })?;
    println!("{}\n{}", run.run_id, run.command_line);

    let t0 = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(400));
        match engine.status() {
            EngineStatus::Loading { .. } if t0.elapsed() < Duration::from_secs(1800) => continue,
            EngineStatus::Ready { load_ms, ctx_served, .. } => {
                println!("PRONTO in {load_ms} ms · n_ctx {ctx_served:?} · {}", run.base_url);
                break;
            }
            other => {
                let _ = engine.stop();
                return Err(format!("non è arrivato a pronto: {other:?}"));
            }
        }
    }

    let stop = root.path.join("stop-m08");
    let _ = std::fs::remove_file(&stop);
    println!("acceso: fermalo creando {}", stop.display());
    while !stop.exists() {
        if !engine.is_running() {
            return Err(format!("il motore è uscito da solo: {:?}", engine.status()));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    let _ = std::fs::remove_file(&stop);
    engine.stop()?;
    println!("fermato · run {run_id}");
    Ok(())
}
