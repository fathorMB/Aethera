//! Prova di M-05 T-07 sulla macchina vera: **da una radice dati vuota a un motore acceso** senza
//! scrivere a mano nessun file.
//!
//! Uso: `cargo run --example e2e_m05 -- <radice-esistente> [--keep] [--no-start]`
//!
//! La radice esistente serve solo come dispensa: da lì si prendono la cartella di una build vera e
//! il percorso dei pesi veri. Tutto il resto nasce vuoto in una cartella temporanea e viene
//! riempito con gli stessi gesti che fa la finestra: dichiarare la build, registrare i pesi che
//! stanno fuori (hard link, non copia), farsi scrivere un profilo nuovo dal modello, avviarlo.
//!
//! `--no-start` si ferma prima di accendere il motore (utile quando la VRAM è occupata);
//! `--keep` lascia la radice temporanea dov'è invece di cancellarla.

use aethera_lib::catalog::{self, ScanInput};
use aethera_lib::clients::{self, RunFacts};
use aethera_lib::engine::{Engine, EngineStatus, StartRequest};
use aethera_lib::machine::{BuildEntry, DataRoot};
use aethera_lib::{adopt, launch, modelcard, profile, system};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

/// Repository del modello, per la model card: la voce scoperta sul disco non lo sa da sola.
const REPO: &str = "bartowski/Qwen_Qwen3.6-35B-A3B-GGUF";

fn check(ok: bool, what: &str, failures: &mut Vec<String>) {
    println!("{} {what}", if ok { "✓" } else { "✗" });
    if !ok {
        failures.push(what.to_string());
    }
}

fn gb(bytes: u64) -> f64 {
    bytes as f64 / 1e9
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let source_root = DataRoot::new(args.first().ok_or("uso: e2e_m05 <radice-esistente> [--keep] [--no-start]")?);
    let keep = args.iter().any(|a| a == "--keep");
    let start_engine = !args.iter().any(|a| a == "--no-start");
    let source_machine = source_root.load_machine()?;
    let probe = system::probe();
    let mut failures = Vec::new();

    let weights_source = source_machine
        .models_dir
        .clone()
        .ok_or("la radice di partenza non dichiara la cartella dei pesi")?;
    let gguf: PathBuf = std::fs::read_dir(&weights_source)
        .map_err(|e| format!("{}: {e}", weights_source.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().to_ascii_lowercase().ends_with(".gguf"))
        // Il più grande: è il modello vero, non la testa MTP che gli sta accanto.
        .max_by_key(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
        .ok_or("nessun .gguf nella cartella dei pesi di partenza")?;
    let build_dir = source_root
        .builds_available(&source_machine)
        .first()
        .map(|b| b.dir.clone())
        .ok_or("nessuna build installata nella radice di partenza")?;

    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let root = DataRoot::new(std::env::temp_dir().join(format!("aethera-m05-{stamp}")));
    println!("radice dati nuova: {}", root.path.display());
    println!("pesi veri:         {} ({:.2} GB)", gguf.display(), gb(std::fs::metadata(&gguf).map(|m| m.len()).unwrap_or(0)));
    println!("build vera:        {}", build_dir.display());

    // --- 1. Radice vuota: la scelta della cartella è l'unica cosa che fa l'operatore ---
    println!("\n=== radice dati vuota ===");
    let mut machine = root.ensure(&probe.report())?;
    check(root.machine_file().is_file(), "machine.toml scritto da solo alla prima apertura", &mut failures);
    for d in [root.profiles(), root.builds(), root.runs()] {
        check(d.is_dir(), &format!("{} creata", d.display()), &mut failures);
    }
    check(machine.models_dir.is_none(), "nessuna cartella dei pesi inventata", &mut failures);
    check(machine.builds.is_empty(), "nessuna build inventata", &mut failures);
    check(
        std::fs::read_dir(root.profiles()).map(|d| d.count()).unwrap_or(1) == 0,
        "nessun profilo: si parte davvero da zero",
        &mut failures,
    );

    // --- 2. «Importa cartella…»: una build già scaricata si dichiara senza toccare machine.toml ---
    println!("\n=== build dichiarata dalla finestra ===");
    let name = build_dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let id = name.strip_prefix("llama-").unwrap_or(&name).to_string();
    check(
        build_dir.join(aethera_lib::machine::server_binary_name()).is_file(),
        "la cartella contiene llama-server",
        &mut failures,
    );
    machine.builds.push(BuildEntry { id: id.clone(), path: build_dir.clone() });
    let weights_dir = root.path.join("pesi");
    std::fs::create_dir_all(&weights_dir).map_err(|e| e.to_string())?;
    machine.models_dir = Some(weights_dir.clone());
    root.save_machine(&machine)?;
    let machine = root.load_machine()?;
    let builds = root.builds_available(&machine);
    check(builds.iter().any(|b| b.id == id), &format!("build «{id}» trovata dopo il salvataggio"), &mut failures);
    let (build_tag, backend) = aethera_lib::machine::split_build_id(&id);
    println!("id «{id}» → build {build_tag:?} backend {backend:?}");
    check(build_tag.is_some() && backend.is_some(), "build e backend ricavati dall'id della cartella", &mut failures);

    // --- 3. «Importa da disco…»: pesi che stanno fuori, collegati e non copiati ---
    println!("\n=== pesi registrati da fuori la cartella ===");
    let links_before = probe.hard_links(&gguf);
    let free_before = probe.free_disk_bytes(&weights_dir);
    let plan = adopt::plan(&gguf, &weights_dir, probe.as_ref())?;
    println!("piano: {:?} · {} → {}", plan.action, plan.source, plan.target);
    check(plan.action == adopt::Action::Link, "stesso volume: hard link, non copia", &mut failures);
    check(plan.blocker.is_none(), "niente impedisce la registrazione", &mut failures);
    let outcome = adopt::apply(&plan, false, &AtomicBool::new(false), &mut |_, _| {})?;
    check(matches!(outcome, adopt::Outcome::Linked { .. }), "pesi collegati nella cartella", &mut failures);
    let links_after = probe.hard_links(&gguf);
    let free_after = probe.free_disk_bytes(&weights_dir);
    println!("collegamenti {links_before:?} → {links_after:?}");
    check(
        matches!((links_before, links_after), (Some(a), Some(b)) if b == a + 1),
        "un collegamento in più sullo stesso contenuto",
        &mut failures,
    );
    if let (Some(a), Some(b)) = (free_before, free_after) {
        let spent = a.saturating_sub(b);
        println!("spazio libero {:.1} GB → {:.1} GB (differenza {:.2} GB)", gb(a), gb(b), gb(spent));
        check(spent < 1_000_000_000, "il disco non si è riempito: nessun byte duplicato", &mut failures);
    }
    // Rifarlo non fa un doppione: lo stesso file è riconosciuto per identità, non per nome.
    let again = adopt::plan(&gguf, &weights_dir, probe.as_ref())?;
    check(again.action == adopt::Action::Nothing, "registrarlo di nuovo non duplica niente", &mut failures);
    println!("seconda volta: {}", again.notes.join(" · "));

    // --- 4. Il catalogo lo trova da solo e ne legge i metadati ---
    println!("\n=== catalogo ===");
    let mut cat = catalog::load(&root)?;
    let empty = std::collections::BTreeMap::new();
    let rows = catalog::scan(
        &mut cat,
        &ScanInput { machine: &machine, profiles: &[], compute: &empty, probe: probe.as_ref() },
    );
    catalog::save(&root, &cat)?;
    let file = gguf.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let row = rows.iter().find(|r| r.file == file).ok_or("il catalogo non ha trovato i pesi appena collegati")?;
    println!("voce «{}» · {:?} · hard link {:?}", row.id, row.state, row.hard_links);
    check(row.info.is_some(), "metadati GGUF letti senza caricare il modello", &mut failures);
    check((row.hard_links.unwrap_or(1)) > 1, "il catalogo dichiara che il file è collegato altrove", &mut failures);

    // --- 5. Campionamento dalla model card, con fonte e data (T-06) ---
    println!("\n=== campionamento dalla model card ===");
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let sampling = match modelcard::from_repo(REPO, &today) {
        Ok(found) => {
            for n in &found.notes {
                println!("  {n}");
            }
            for (mode, s) in &found.sampling {
                println!("  {mode}: temp {:?} top_p {:?} top_k {:?}", s.temperature, s.top_p, s.top_k);
            }
            check(!found.sampling.is_empty(), "il campionamento consigliato è stato letto", &mut failures);
            check(
                found.sampling.values().all(|s| s.source.is_some() && s.verified.is_some()),
                "ogni modalità porta fonte e data di lettura",
                &mut failures,
            );
            check(
                found.sampling.values().all(|s| s.source.as_deref().is_some_and(|x| !x.contains("bartowski"))),
                "la fonte è la model card del modello originale, non quella della riquantizzazione",
                &mut failures,
            );
            found.sampling
        }
        // Senza rete la prova continua: il resto non dipende da Hugging Face.
        Err(e) => {
            println!("  model card non letta ({e}): il resto della prova continua");
            Default::default()
        }
    };
    if let Some(entry) = cat.models.iter_mut().find(|e| e.file == file) {
        entry.repo = Some(REPO.to_string());
        entry.sampling_by_mode = sampling.clone();
    }
    catalog::save(&root, &cat)?;

    // --- 6. Profilo nuovo scritto dalla finestra, non a mano ---
    println!("\n=== profilo nuovo ===");
    let entry = cat.models.iter().find(|e| e.file == file).cloned().ok_or("voce di catalogo sparita")?;
    let mut p = profile::template(
        &profile::name_from_file(&file),
        &file,
        build_tag.as_deref().unwrap_or_default(),
        backend.as_deref().unwrap_or("vulkan"),
        8080,
    );
    p.model.repo = entry.repo.clone();
    p.model.sha256 = entry.sha256_verified.clone().or(entry.sha256.clone());
    p.model.quant = entry.gguf.as_ref().and_then(|g| g.info.dominant_type.clone());
    p.sampling_by_mode = entry.sampling_by_mode.clone();
    let issues = profile::validate(&p);
    println!("profilo «{}» · {} errori", p.name, issues.len());
    check(issues.is_empty(), "il profilo nuovo è valido senza toccare niente", &mut failures);
    check(p.name == p.name.to_lowercase() && !p.name.is_empty(), "alias uguale al nome", &mut failures);

    let file_path = root.profiles().join(format!("{}.toml", p.name));
    std::fs::write(&file_path, toml::to_string_pretty(&p).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let back = std::fs::read_to_string(&file_path).map_err(|e| e.to_string())?;
    let (reread, issues) = profile::load(&back, &p.name);
    check(issues.is_empty() && reread.as_ref() == Some(&p), "riletto dal disco è identico", &mut failures);

    // Duplicare, rinominare, cancellare (T-02) sugli stessi gesti dei comandi della finestra.
    let mut copy = p.clone();
    copy.name = format!("{}-2", p.name);
    let copy_path = root.profiles().join(format!("{}.toml", copy.name));
    std::fs::write(&copy_path, toml::to_string_pretty(&copy).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    check(profile::validate(&copy).is_empty(), "il duplicato è valido e ha il suo alias", &mut failures);
    std::fs::remove_file(&copy_path).map_err(|e| e.to_string())?;
    check(!copy_path.exists(), "il duplicato si cancella senza toccare l'originale", &mut failures);
    check(file_path.is_file(), "l'originale è ancora lì", &mut failures);

    // --- 7. Avvio ---
    println!("\n=== avvio ===");
    let run_id = chrono::Local::now().format("r-%Y%m%d-%H%M%S").to_string();
    let run_dir = root.runs().join(&run_id);
    let prep = launch::prepare(&root, &machine, &p.name, &p, &run_dir.join("slots"));
    for b in &prep.blockers {
        println!("  blocco: {b}");
    }
    check(prep.blockers.is_empty(), "niente impedisce l'avvio del profilo appena creato", &mut failures);
    check(prep.build.is_some() && prep.model_path.is_some(), "build e pesi risolti sulla macchina", &mut failures);

    if start_engine && prep.blockers.is_empty() {
        let engine = Engine::default();
        let run = engine.start(StartRequest {
            run_id: run_id.clone(),
            run_dir: run_dir.clone(),
            base: p.name.clone(),
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
            system: probe.report(),
        })?;
        println!("avviato {} pid {} · {}", run.run_id, run.pid, run.command_line);

        let t = Instant::now();
        let mut ready = None;
        while t.elapsed() < Duration::from_secs(600) {
            match engine.status() {
                EngineStatus::Ready { load_ms, ctx_served, alias_served, divergences, .. } => {
                    ready = Some((load_ms, ctx_served, alias_served, divergences));
                    break;
                }
                EngineStatus::Exited { finished } => {
                    return Err(format!("il motore è uscito con codice {:?}", finished.code));
                }
                _ => std::thread::sleep(Duration::from_millis(500)),
            }
        }
        let (load_ms, ctx_served, alias_served, divergences) =
            ready.ok_or("il motore non è diventato pronto entro 10 minuti")?;
        println!("pronto in {load_ms} ms · ctx servito {ctx_served:?} · alias {alias_served:?}");
        check(alias_served.as_deref() == Some(p.name.as_str()), "l'alias servito è il nome del profilo", &mut failures);
        check(ctx_served == Some(p.server.ctx), "il contesto servito è quello dichiarato", &mut failures);
        check(divergences.is_empty(), "nessuna divergenza fra dichiarato e servito", &mut failures);

        // --- 8. Il campionamento arriva fino ai frammenti per i client (T-06) ---
        let snip = clients::snippets(&RunFacts {
            run_id: &run.run_id,
            base_url: &run.base_url,
            alias: &p.name,
            ctx_declared: p.server.ctx,
            ctx_served,
            client: p.client.as_ref(),
            endpoint: None,
            sampling: &p.sampling_by_mode,
        });
        println!("\n--- profile.toml per i client ---\n{}", snip.toml);
        if !p.sampling_by_mode.is_empty() {
            check(snip.toml.contains("[sampling."), "le righe per i client portano il campionamento", &mut failures);
            check(snip.toml.contains("# fonte:"), "con la fonte scritta accanto", &mut failures);
            check(snip.env.contains("# campionamento"), "anche il blocco d'ambiente lo dice", &mut failures);
        }

        engine.stop()?;
        std::thread::sleep(Duration::from_secs(2));
        check(!engine.is_running(), "motore fermato", &mut failures);
    } else {
        println!("(avvio saltato)");
    }

    // --- Pulizia: si cancella solo la radice temporanea, e il collegamento non porta via i pesi ---
    if !keep {
        let in_temp = root.path.starts_with(std::env::temp_dir());
        if in_temp {
            let _ = std::fs::remove_dir_all(&root.path);
        }
        check(in_temp && !root.path.exists(), "radice temporanea rimossa", &mut failures);
        check(gguf.is_file(), "i pesi originali sono ancora al loro posto", &mut failures);
        check(
            std::fs::metadata(&gguf).map(|m| m.len()).unwrap_or(0) > 0,
            "e non sono stati svuotati dalla cancellazione del collegamento",
            &mut failures,
        );
    } else {
        println!("radice lasciata in {}", root.path.display());
    }

    println!();
    if failures.is_empty() {
        println!("tutto verde");
        Ok(())
    } else {
        Err(format!("{} controlli falliti:\n  {}", failures.len(), failures.join("\n  ")))
    }
}
