//! M-20: accende i motori di servizio dichiarati in `profiles/` e li tiene su finché non compare
//! il file di stop. Serve a provare il percorso vero — `services::Services::start`, la riga di
//! comando di `service::build_args`, il job object, l'attesa di `/health`, la VRAM per processo —
//! invece di lanciare `llama-server` a mano e illudersi di aver provato l'app.
//!
//! Uso: `cargo run --release --example m20_services -- <radice> [nome ...]`
//! Si ferma quando compare `<radice>/stop-m20` (o con Ctrl+C: il job object chiude i figli).

use aethera_lib::machine::{self, DataRoot};
use aethera_lib::service;
use aethera_lib::services::{self, Services, StartService};
use aethera_lib::system;
use std::time::Duration;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = DataRoot::new(args.first().ok_or("uso: m20_services <radice> [nome ...]")?);
    let voluti: Vec<String> = args[1..].to_vec();

    let machine = root.load_machine()?;
    let models_dir = machine.models_dir.clone().ok_or("cartella dei pesi non impostata in machine.toml")?;
    let builds = root.builds_available(&machine);

    let profili = services::list_profiles(&root.profiles());
    if profili.is_empty() {
        return Err(format!("nessun profilo di servizio in {}", root.profiles().display()));
    }

    let s = Services::default();
    let stop = root.path.join("stop-m20");
    let _ = std::fs::remove_file(&stop);

    for (p, issues) in &profili {
        if !voluti.is_empty() && !voluti.contains(&p.name) {
            continue;
        }
        if !issues.is_empty() {
            println!("· {} SALTATO: {} — {}", p.name, issues[0].field, issues[0].message);
            continue;
        }
        let build = match machine::resolve_build(&builds, &p.runtime.build, &p.runtime.backend) {
            Some(b) => b,
            None => {
                println!("· {} SALTATO: build «{} {}» non dichiarata", p.name, p.runtime.build, p.runtime.backend);
                continue;
            }
        };
        let model = service::model_path(&models_dir, p);
        match s.start(StartService { profile: p.clone(), binary: build.binary.clone(), model }) {
            Ok(v) => {
                println!("\n· {} PRONTO in {} ms su {}", v.name, v.ready_ms, v.base_url);
                println!("  {}", v.command_line);
                if v.kind == "rerank" {
                    match services::rerank_sanity(&v.base_url) {
                        // La soglia e' la stessa del comando dell'app: sotto, il GGUF e' da rifare.
                        Ok(score) => {
                            let sano = score > 0.5;
                            s.set_sane(&v.name, sano);
                            println!("  test di sanita' del reranker: {score:.3} sul documento pertinente — {}",
                                     if sano { "SANO" } else { "NON ATTENDIBILE, il GGUF e' probabilmente convertito male" });
                        }
                        Err(e) => {
                            s.set_sane(&v.name, false);
                            println!("  test di sanita' del reranker FALLITO: {e}");
                        }
                    }
                }
            }
            Err(e) => println!("· {} NON PARTITO: {e}", p.name),
        }
    }

    if s.names().is_empty() {
        return Err("nessun servizio acceso".into());
    }

    // La memoria si legge dopo che tutti sono su: e' quella che conta per sapere quanto costano.
    std::thread::sleep(Duration::from_secs(3));
    let probe = system::probe();
    println!("\nmemoria dei servizi (contatore per processo):");
    let mut somma = 0.0;
    for v in s.view(probe.as_ref()) {
        let g = v.vram_dedicated_gib.unwrap_or(0.0);
        somma += g;
        println!("  {:28} pid {:6} {:>6.2} GiB  {}", v.name, v.pid, g, v.state);
    }
    println!("  {:28} {:11} {:>6.2} GiB in tutto", "", "", somma);

    // L'endpoint di Aethera, per provare davvero che i servizi escono da GET /services e da
    // /status. Il motore principale qui e' tenuto da m08_hold, quindi la parte «engine» dira'
    // «off»: e' l'array dei servizi che si sta verificando, e va detto invece di confondere.
    match aethera_lib::endpoint::spawn(
        aethera_lib::engine::Engine::default(),
        s.clone(),
        aethera_lib::endpoint::DEFAULT_ADDR,
    ) {
        Ok(a) => println!("\nendpoint su http://{a} (lo stato del motore dira' «off»: non e' questo processo a tenerlo)"),
        Err(e) => println!("\nendpoint non partito: {e}"),
    }

    println!("\nacceso: fermali creando {}", stop.display());
    while !stop.exists() {
        std::thread::sleep(Duration::from_millis(500));
    }
    s.stop_all();
    let _ = std::fs::remove_file(&stop);
    println!("fermati.");
    Ok(())
}
