//! Prova di M-06 sulla macchina vera: che cosa succede quando qualcosa va storto.
//!
//! Uso: `cargo run --example e2e_m06 -- <radice-esistente> [--keep]`
//!
//! La radice esistente è solo la dispensa da cui si prendono una build vera e i pesi veri. I
//! guasti si provocano in una radice temporanea: un `catalog.toml` illeggibile, pesi rinominati
//! sotto il naso del catalogo, una build dichiarata e poi sparita, un motore che esce con errore.
//! Niente di quello che succede qui tocca la radice dell'operatore.

use aethera_lib::catalog::{self, ScanInput, State};
use aethera_lib::engine::{Engine, EngineStatus, StartRequest};
use aethera_lib::machine::{BuildEntry, DataRoot};
use aethera_lib::{adopt, diagnose, launch, profile, system};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

fn check(ok: bool, what: &str, failures: &mut Vec<String>) {
    println!("{} {what}", if ok { "✓" } else { "✗" });
    if !ok {
        failures.push(what.to_string());
    }
}

fn scan(root: &DataRoot, machine: &aethera_lib::machine::MachineConfig, probe: &dyn system::SystemProbe) -> Vec<catalog::ModelRow> {
    let mut cat = catalog::load(root).unwrap_or_default();
    let empty = BTreeMap::new();
    catalog::scan(&mut cat, &ScanInput { machine, profiles: &[], compute: &empty, probe })
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let source = DataRoot::new(args.first().ok_or("uso: e2e_m06 <radice-esistente> [--keep]")?);
    let keep = args.iter().any(|a| a == "--keep");
    let source_machine = source.load_machine()?;
    let probe = system::probe();
    let mut failures = Vec::new();

    let gguf: PathBuf = std::fs::read_dir(source_machine.models_dir.clone().ok_or("niente cartella dei pesi")?)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().to_ascii_lowercase().ends_with(".gguf"))
        .max_by_key(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
        .ok_or("nessun .gguf")?;
    let build_dir = source.builds_available(&source_machine).first().map(|b| b.dir.clone()).ok_or("nessuna build")?;

    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let root = DataRoot::new(std::env::temp_dir().join(format!("aethera-m06-{stamp}")));
    println!("radice dei guasti: {}", root.path.display());
    let mut machine = root.ensure(&probe.report())?;
    let weights = root.path.join("pesi");
    std::fs::create_dir_all(&weights).map_err(|e| e.to_string())?;
    machine.models_dir = Some(weights.clone());
    machine.builds.push(BuildEntry { id: "b10809-vulkan".into(), path: build_dir.clone() });
    root.save_machine(&machine)?;
    let machine = root.load_machine()?;

    // I pesi arrivano per collegamento: nessun byte copiato, e cancellare la radice non li tocca.
    let plan = adopt::plan(&gguf, &weights, probe.as_ref())?;
    adopt::apply(&plan, false, &AtomicBool::new(false), &mut |_, _| {})?;
    let file = plan.file.clone();

    // --- T-01: catalog.toml illeggibile ---
    println!("\n=== catalog.toml illeggibile ===");
    let cat_path = catalog::path(&root);
    let garbage = "questo non e' TOML [[[\nschema_version = \"uno\"\n";
    std::fs::write(&cat_path, garbage).map_err(|e| e.to_string())?;
    match catalog::load(&root) {
        Ok(_) => check(false, "un catalogo illeggibile deve essere un errore, non un catalogo vuoto", &mut failures),
        Err(e) => {
            println!("  {e}");
            check(e.contains("catalog.toml"), "il messaggio dice quale file", &mut failures);
            check(e.len() > "catalog.toml".len() + 5, "e perché", &mut failures);
        }
    }
    // Il resto continua a funzionare: la scansione vede lo stesso i file sul disco…
    let rows = scan(&root, &machine, probe.as_ref());
    check(rows.iter().any(|r| r.file == file), "i pesi sul disco si vedono lo stesso", &mut failures);
    // …e il file illeggibile non è stato riscritto.
    check(
        std::fs::read_to_string(&cat_path).unwrap_or_default() == garbage,
        "il catalogo illeggibile non viene sovrascritto",
        &mut failures,
    );
    std::fs::remove_file(&cat_path).map_err(|e| e.to_string())?;

    // --- T-01 (seconda metà): un profilo illeggibile non ferma gli altri ---
    println!("\n=== profilo illeggibile ===");
    let good = profile::template("buono", &file, "b10809", "vulkan", 8080);
    std::fs::write(root.profiles().join("buono.toml"), toml::to_string_pretty(&good).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    std::fs::write(root.profiles().join("rotto.toml"), "ctx = questo non e' un valore\n").map_err(|e| e.to_string())?;
    let (parsed, issues) = profile::load("ctx = questo non e' un valore\n", "rotto");
    println!("  {}", issues.first().map(|i| i.message.clone()).unwrap_or_default());
    check(parsed.is_none() && !issues.is_empty(), "il profilo rotto dà un motivo, non un silenzio", &mut failures);
    let (still, ok_issues) =
        profile::load(&std::fs::read_to_string(root.profiles().join("buono.toml")).map_err(|e| e.to_string())?, "buono");
    check(still.is_some() && ok_issues.is_empty(), "il profilo valido accanto continua a funzionare", &mut failures);

    // --- T-02: pesi rinominati sotto il naso del catalogo ---
    println!("\n=== pesi rinominati ===");
    let mut cat = catalog::load(&root).unwrap_or_default();
    let size_gb = std::fs::metadata(&weights.join(&file)).map(|m| m.len() as f64 / 1e9).unwrap_or(0.0);
    let mut entry = catalog::Entry::discovered(&file);
    entry.size_gb = Some((size_gb * 100.0).round() / 100.0);
    entry.repo = Some("prova/repo".into());
    entry.sha256 = Some("0".repeat(64));
    entry.sha256_verified = Some("0".repeat(64));
    let id = entry.id.clone();
    cat.models = vec![entry];
    catalog::save(&root, &cat)?;

    let renamed = format!("rinominato-{file}");
    std::fs::rename(weights.join(&file), weights.join(&renamed)).map_err(|e| e.to_string())?;
    let rows = scan(&root, &machine, probe.as_ref());
    let lost = rows.iter().find(|r| r.id == id).ok_or("la voce è sparita invece di dirsi mancante")?;
    println!("  «{}» → {:?}, candidati {:?}", lost.id, lost.state, lost.renamed_candidates);
    check(lost.state == State::Downloadable, "la voce dice che i pesi non ci sono più", &mut failures);
    check(
        lost.renamed_candidates.iter().any(|c| *c == renamed),
        "e indica il file presente che pesa quanto lei: forse è lo stesso rinominato",
        &mut failures,
    );
    // Ricollegare butta l'hash vecchio: valeva per un altro file.
    let mut cat = catalog::load(&root)?;
    if let Some(e) = cat.models.iter_mut().find(|e| e.id == id) {
        e.file = renamed.clone();
        e.sha256_verified = None;
        e.verified_at = None;
        e.gguf = None;
    }
    catalog::save(&root, &cat)?;
    let rows = scan(&root, &machine, probe.as_ref());
    let back = rows.iter().find(|r| r.id == id).ok_or("voce sparita dopo il ricollegamento")?;
    check(back.state == State::Present, "dopo il ricollegamento la voce è di nuovo presente", &mut failures);
    check(back.sha256_verified.is_none(), "l'hash calcolato sul file vecchio non viene portato dietro", &mut failures);
    std::fs::rename(weights.join(&renamed), weights.join(&file)).map_err(|e| e.to_string())?;

    // --- T-02 (seconda metà): build dichiarata ma sparita dal disco ---
    println!("\n=== build dichiarata ma sparita ===");
    let mut broken = machine.clone();
    broken.builds = vec![BuildEntry { id: "b10809-vulkan".into(), path: root.path.join("build-che-non-ce") }];
    let p = profile::template("prova", &file, "b10809", "vulkan", 8080);
    let prep = launch::prepare(&root, &broken, "", &p, &root.runs().join("x"));
    let msg = prep.blockers.join(" | ");
    println!("  {msg}");
    check(msg.contains("dichiarata in machine.toml"), "si dice che era dichiarata, non che non esiste", &mut failures);
    check(msg.contains("build-che-non-ce"), "e dove la cercava", &mut failures);

    let mut never = machine.clone();
    never.builds.clear();
    let p2 = profile::template("prova", &file, "b99999", "vulkan", 8080);
    let msg2 = launch::prepare(&root, &never, "", &p2, &root.runs().join("x")).blockers.join(" | ");
    println!("  {msg2}");
    check(msg2.contains("nessuna build"), "una build mai dichiarata è un guasto diverso", &mut failures);

    // --- T-05: motore che esce con errore, e le righe che lo spiegano ---
    println!("\n=== motore che esce con errore ===");
    let mut bad = profile::template("con-una-leva-che-non-esiste", &file, "b10809", "vulkan", 8080);
    bad.server.extra_args = vec!["--questa-leva-non-esiste".into()];
    let prep = launch::prepare(&root, &machine, "", &bad, &root.runs().join("r-guasto").join("slots"));
    check(prep.blockers.is_empty(), "una leva sconosciuta non si può sapere prima: l'avvio parte", &mut failures);
    let engine = Engine::default();
    engine.start(StartRequest {
        run_id: "r-guasto".into(),
        run_dir: root.runs().join("r-guasto"),
        base: String::new(),
        profile: bad.clone(),
        profile_file: None,
        overrides: prep.overrides,
        invalidates_cache: false,
        build: prep.build.expect("build"),
        model_path: prep.model_path.expect("pesi"),
        model_size: prep.model_size.unwrap_or(0),
        args: prep.args,
        machine_name: machine.name.clone(),
        ram_margin_gib: machine.ram_margin_gib,
        system: probe.report(),
        conditions: system::probe().conditions(None),
        reference: None,
        conditions_changed: Vec::new(),
    })?;
    let t = Instant::now();
    let mut code = None;
    while t.elapsed() < Duration::from_secs(60) {
        if let EngineStatus::Exited { finished } = engine.status() {
            code = Some(finished.code);
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    check(code.is_some(), "il motore esce con errore invece di restare in caricamento per sempre", &mut failures);
    println!("  codice di uscita {code:?}");
    let log = engine.log_path().and_then(|p| aethera_lib::engine::read_tail(&p, 400).ok()).unwrap_or_default();
    let reasons = diagnose::engine_failure(&log, 6);
    for r in &reasons {
        println!("  · {}", r.line);
        if let Some(h) = &r.hint {
            println!("      → {h}");
        }
    }
    check(!reasons.is_empty(), "le righe che spiegano il perché vengono messe in evidenza", &mut failures);
    check(
        reasons.iter().any(|r| r.hint.as_deref().is_some_and(|h| h.contains("build"))),
        "e una leva sconosciuta dice che è la build a non conoscerla",
        &mut failures,
    );

    // --- T-07: radice dati su un disco che non c'è, e senza permessi ---
    println!("\n=== radice dati irraggiungibile ===");
    let ghost = DataRoot::new(r"Z:\Aethera-che-non-esiste");
    match std::fs::create_dir_all(&ghost.path) {
        Err(e) => {
            let msg = diagnose::write_error("creazione", &ghost.path, &e);
            println!("  {msg}");
            check(msg.contains(r"Z:\Aethera-che-non-esiste"), "si dice quale percorso", &mut failures);
            check(diagnose::io_hint(&e).is_some(), "e che cosa farne, non il solo codice di errore", &mut failures);
        }
        // Se per caso Z: esiste su questa macchina, il caso non si può provocare: si dice.
        Ok(_) => println!("  (Z: esiste su questa macchina: caso non provocabile, saltato)"),
    }
    let unwritable = DataRoot::new(root.path.join("mai-creata"));
    match unwritable.check_writable() {
        Err(e) => {
            println!("  {e}");
            check(e.contains("mai-creata"), "una cartella che non c'è si dice per nome", &mut failures);
        }
        Ok(_) => check(false, "una cartella che non esiste non può risultare scrivibile", &mut failures),
    }
    check(root.check_writable().is_ok(), "una radice buona resta scrivibile", &mut failures);

    if !keep && root.path.starts_with(std::env::temp_dir()) {
        let _ = std::fs::remove_dir_all(&root.path);
        check(gguf.is_file(), "i pesi originali sono ancora al loro posto", &mut failures);
    } else {
        println!("\nradice lasciata in {}", root.path.display());
    }

    println!();
    if failures.is_empty() {
        println!("tutto verde");
        Ok(())
    } else {
        Err(format!("{} controlli falliti:\n  {}", failures.len(), failures.join("\n  ")))
    }
}
