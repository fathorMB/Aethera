//! Prova di M-04 T-09 sulla macchina vera: metadati dei GGUF presenti, stima confrontata con la
//! misura del G1, SHA-256 contro l'oid LFS, ripresa di un download interrotto, release di build
//! con il digest, hard link riconosciuti.
//!
//! Uso: `cargo run --example e2e_m04 -- <radice-dati> [--hash]`
//! `--hash` aggiunge il ricalcolo completo dello SHA-256 dei 22 GB (minuti): senza, si verifica
//! solo su un pezzo del file che il calcolo è avviabile e annullabile.
//!
//! Non scarica interi modelli: la ripresa si prova sul file vero fermandola dopo pochi MB.

use aethera_lib::catalog::{self, ScanInput, State};
use aethera_lib::download::{self, Outcome, Plan, Request};
use aethera_lib::machine::DataRoot;
use aethera_lib::{builds, estimate, gguf, hash, profile, system};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

const G1_FILE: &str = "Qwen_Qwen3.6-35B-A3B-Q4_K_M.gguf";
const G1_REPO: &str = "bartowski/Qwen_Qwen3.6-35B-A3B-GGUF";
const G1_OID: &str = "b46fedd33e0bfb0cae308aa3c158d0a4b2c4a1d2185a1ed6f093cdaf39064772";
const G1_BYTES: u64 = 22_285_080_192;
/// VRAM dedicata misurata sul G1 il 15/16-09, come `serve.ps1`.
const G1_MEASURED_GIB: f64 = 22.68;

fn check(ok: bool, what: &str, failures: &mut Vec<String>) {
    println!("{} {what}", if ok { "✓" } else { "✗" });
    if !ok {
        failures.push(what.to_string());
    }
}

fn read_profiles(root: &DataRoot) -> Vec<profile::Profile> {
    let Ok(dir) = std::fs::read_dir(root.profiles()) else { return Vec::new() };
    dir.flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .filter_map(|p| {
            let stem = p.file_stem()?.to_string_lossy().to_string();
            profile::load(&std::fs::read_to_string(&p).ok()?, &stem).0
        })
        .collect()
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = DataRoot::new(args.first().ok_or("uso: e2e_m04 <radice-dati> [--hash]")?);
    let full_hash = args.iter().any(|a| a == "--hash");
    let machine = root.load_machine()?;
    let probe = system::probe();
    let mut failures = Vec::new();
    let models_dir = machine.models_dir.clone().ok_or("machine.toml non dichiara la cartella dei pesi")?;

    // --- T-01: metadati dal GGUF vero, senza caricare il modello ---
    println!("\n=== metadati GGUF ===");
    let g1_path = models_dir.join(G1_FILE);
    let t = Instant::now();
    let info = gguf::read_info(&g1_path)?;
    let read_ms = t.elapsed().as_millis();
    println!("{G1_FILE}: letto in {read_ms} ms · {info:?}");
    check(info.arch.as_deref() == Some("qwen35moe"), "architettura qwen35moe", &mut failures);
    check(info.block_count == Some(41), "41 blocchi", &mut failures);
    check(info.context_train == Some(262_144), "contesto di training 262.144", &mut failures);
    check(info.mtp_layers == Some(1), "1 layer MTP dichiarato", &mut failures);
    check(info.full_attention_interval == Some(4), "attenzione piena ogni 4 blocchi", &mut failures);
    check(info.mtp_types.iter().any(|t| t == "Q8_0"), "tensori MTP già in Q8_0", &mut failures);
    check(read_ms < 10_000, "intestazione letta in meno di 10 s su 22 GB", &mut failures);

    // --- T-02: stima confrontata con la misura vera ---
    println!("\n=== stima di memoria ===");
    let profiles = read_profiles(&root);
    let g1 = profiles
        .iter()
        .find(|p| p.model.file == G1_FILE)
        .ok_or("nessun profilo usa i pesi del G1: importa i profili di M-02")?;
    let info_for = |file: &str| (file == G1_FILE).then(|| info.clone());
    let compute = catalog::measured_compute(&root.runs(), &info_for);
    println!("buffer misurati dagli avvii: {compute:?}");
    let key = catalog::compute_key(g1.server.ubatch, &g1.runtime.backend);
    let with = estimate::estimate(
        Some(&info),
        &g1.server,
        Some(G1_BYTES),
        compute.get(&key).map(|m| estimate::MeasuredCompute { run_id: m.run_id.clone(), bytes: m.bytes }),
    );
    println!("stima con ctx {}: {with:?}", g1.server.ctx);
    check(with.kv_bytes.is_some(), "cache KV calcolata dai metadati", &mut failures);
    check(with.full_attention_blocks == Some(10), "KV contata solo sui 10 blocchi ad attenzione piena", &mut failures);
    let measured_bytes = (G1_MEASURED_GIB * (1u64 << 30) as f64) as u64;
    if let Some(total) = with.total_bytes.filter(|_| !with.total_is_lower_bound) {
        // Attenzione a come si legge questo numero: il buffer è stato ricavato da questo stesso
        // avvio sottraendo pesi, KV e stato dalla VRAM misurata. Che la somma torni è un'identità,
        // non una previsione: quello che dimostra è che le formule di KV e stato sono coerenti
        // con la misura, cioè che il residuo attribuito al buffer è plausibile e non assurdo.
        let diff = (total as f64 - measured_bytes as f64).abs() / measured_bytes as f64;
        println!("somma delle voci {:.2} GB · misurato {:.2} GB · scarto {:.2} %", total as f64 / 1e9, measured_bytes as f64 / 1e9, diff * 100.0);
        check(diff < 0.01, "pesi + KV + stato + buffer chiudono con la VRAM misurata (identità: il buffer viene da questo avvio)", &mut failures);
        let buffer = with.compute_bytes.unwrap_or(0);
        println!("buffer residuo attribuito al calcolo: {:.2} GB", buffer as f64 / 1e9);
        check(buffer > 0 && buffer < 4_000_000_000, "il residuo attribuito al buffer è plausibile (fra 0 e 4 GB)", &mut failures);

        // Prova davvero predittiva: lo stesso buffer a contesto doppio deve costare una KV doppia.
        let mut wider = g1.server.clone();
        wider.ctx = g1.server.ctx * 2;
        let at_double = estimate::estimate(
            Some(&info),
            &wider,
            Some(G1_BYTES),
            compute.get(&key).cloned(),
        );
        let (kv1, kv2) = (with.kv_bytes.unwrap_or(0), at_double.kv_bytes.unwrap_or(0));
        println!("KV a {} token: {:.2} GB · a {} token: {:.2} GB", g1.server.ctx, kv1 as f64 / 1e9, wider.ctx, kv2 as f64 / 1e9);
        check(kv2 == kv1 * 2, "raddoppiando il contesto la cache KV raddoppia", &mut failures);
        check(
            at_double.total_bytes.unwrap_or(0) == total + kv1,
            "a contesto doppio la stima cresce esattamente della KV in più, non del buffer",
            &mut failures,
        );
    } else {
        println!("nessun avvio confrontabile (ubatch {} · {}): il totale è un minimo, come previsto", g1.server.ubatch, g1.runtime.backend);
        check(with.total_is_lower_bound, "senza avvio misurato il totale è dichiarato minimo", &mut failures);
    }
    let lower = with.total_bytes.unwrap_or(0);
    check(lower > G1_BYTES, "il totale supera i soli pesi (KV compresa)", &mut failures);

    // --- T-03 e T-08: catalogo, stati e hard link ---
    println!("\n=== catalogo ===");
    let mut cat = catalog::load(&root)?;
    let rows = catalog::scan(&mut cat, &ScanInput { machine: &machine, profiles: &profiles, compute: &compute, probe: probe.as_ref() });
    for r in &rows {
        println!("{:<40} {:?} {:>8.2} GB  link {:?}  profili {:?}", r.id, r.state, r.size_bytes.unwrap_or(0) as f64 / 1e9, r.hard_links, r.profiles);
    }
    let g1_row = rows.iter().find(|r| r.file == G1_FILE).ok_or("il G1 non compare nel catalogo")?;
    check(g1_row.state == State::Present || g1_row.state == State::Verified, "il G1 risulta presente", &mut failures);
    check(g1_row.size_bytes == Some(G1_BYTES), "dimensione sul disco pari a quella dichiarata", &mut failures);
    check(g1_row.info.is_some(), "metadati GGUF nel catalogo", &mut failures);
    check(g1_row.hard_links == Some(2), "i 2 hard link del G1 sono riconosciuti (import di un altro strumento)", &mut failures);
    check(!g1_row.profiles.is_empty(), "il catalogo sa quali profili usano questi pesi", &mut failures);
    check(probe.free_disk_bytes(&models_dir).is_some_and(|b| b > 0), "spazio libero sul volume dei pesi letto", &mut failures);

    // --- T-04: oid LFS dal publisher e SHA-256 ---
    println!("\n=== hash e oid LFS ===");
    let remote = download::remote_file(G1_REPO, G1_FILE)?;
    println!("{remote:?}");
    check(remote.oid.as_deref() == Some(G1_OID), "oid LFS uguale allo SHA-256 noto del G1", &mut failures);
    check(remote.size == Some(G1_BYTES), "dimensione dichiarata dal publisher uguale a quella sul disco", &mut failures);
    check(hash::matches(&format!("sha256:{G1_OID}"), G1_OID), "confronto con il prefisso di GitHub", &mut failures);

    let cancel = AtomicBool::new(true);
    let out = hash::sha256_file(&g1_path, &cancel, &mut |_, _| {})?;
    check(out == hash::Outcome::Cancelled, "il calcolo dello SHA-256 si può annullare subito", &mut failures);
    if full_hash {
        println!("ricalcolo completo dei 22 GB…");
        let t = Instant::now();
        let done = AtomicBool::new(false);
        let mut last = 0u64;
        let out = hash::sha256_file(&g1_path, &done, &mut |d, tot| {
            if d - last > (4u64 << 30) {
                last = d;
                println!("  {:.0} %", d as f64 / tot as f64 * 100.0);
            }
        })?;
        match out {
            hash::Outcome::Done(h) => {
                println!("sha256 {h} in {:.0} s", t.elapsed().as_secs_f64());
                check(hash::matches(G1_OID, &h), "SHA-256 ricalcolato uguale all'oid LFS", &mut failures);
            }
            hash::Outcome::Cancelled => check(false, "ricalcolo completo non annullato", &mut failures),
        }
    } else {
        println!("(ricalcolo completo saltato: ripetere con --hash)");
    }

    // --- T-05: ripresa di un download interrotto, sul file vero ---
    println!("\n=== download riprendibile ===");
    let tmp = std::env::temp_dir().join("aethera-e2e-m04");
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let target = tmp.join(G1_FILE);
    let part = tmp.join(format!("{G1_FILE}{}", catalog::PART_SUFFIX));
    let _ = std::fs::remove_file(&target);
    let _ = std::fs::remove_file(&part);

    let stop_at = 3u64 << 20;
    let flag = AtomicBool::new(false);
    let req = Request {
        url: remote.url.clone(),
        target: target.clone(),
        expected_sha256: remote.oid.clone(),
        expected_size: remote.size,
        free_disk: probe.free_disk_bytes(&tmp),
        cancel: &flag,
    };
    let first = download::download(&req, &mut |done, _| {
        if done >= stop_at {
            flag.store(true, Ordering::Relaxed);
        }
    })?;
    println!("primo tratto: {first:?}");
    let after_first = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    check(matches!(first, Outcome::Paused { .. }), "il download si ferma quando glielo si chiede", &mut failures);
    check(after_first >= stop_at, "il .part contiene quello che è già arrivato", &mut failures);
    check(!target.exists(), "il file finale non esiste finché non è verificato", &mut failures);

    check(download::plan(&target, Some(after_first), remote.size) == Plan::Resume(after_first), "la ripresa riparte dal byte giusto", &mut failures);
    let flag2 = AtomicBool::new(false);
    let stop_at2 = after_first + (2 << 20);
    let req2 = Request {
        url: remote.url.clone(),
        target: target.clone(),
        expected_sha256: remote.oid.clone(),
        expected_size: remote.size,
        free_disk: probe.free_disk_bytes(&tmp),
        cancel: &flag2,
    };
    let second = download::download(&req2, &mut |done, _| {
        if done >= stop_at2 {
            flag2.store(true, Ordering::Relaxed);
        }
    })?;
    let after_second = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    println!("ripresa: {second:?} · .part {after_first} → {after_second} byte");
    check(after_second > after_first, "la ripresa aggiunge byte invece di ricominciare", &mut failures);
    let _ = std::fs::remove_file(&part);
    let _ = std::fs::remove_dir_all(&tmp);

    // --- T-06: release di build con il digest ---
    println!("\n=== build ggml-org ===");
    let rels = builds::releases(8)?;
    let latest = rels.first().ok_or("nessuna release di build trovata")?;
    println!("ultime build: {:?}", rels.iter().map(|r| &r.tag).collect::<Vec<_>>());
    check(rels.iter().all(|r| builds::is_build_tag(&r.tag)), "solo tag di build b<numero>", &mut failures);
    let vulkan = latest.assets.iter().find(|a| a.backend == "win-vulkan-x64").ok_or("nessun asset win-vulkan-x64")?;
    println!("{} · {} · {:.1} MB · {:?}", latest.tag, vulkan.name, vulkan.size as f64 / 1e6, vulkan.digest);
    check(vulkan.digest.as_deref().is_some_and(|d| d.starts_with("sha256:")), "l'asset porta il digest SHA-256", &mut failures);
    check(hash::normalise(vulkan.digest.as_deref().unwrap_or("")).is_some(), "il digest è uno SHA-256 valido", &mut failures);
    check(latest.assets.iter().all(|a| !a.name.starts_with("cudart")), "le librerie CUDA non sono scambiate per build", &mut failures);
    check(builds::dir_name(&latest.tag, &vulkan.backend).starts_with("llama-b"), "cartella di installazione llama-<tag>-<backend>", &mut failures);

    // --- T-06: installazione vera, in una radice temporanea ---
    // Solo con --install: scarica ~32 MB. Va in %TEMP%, non nella radice dati dell'operatore:
    // una prova non deve lasciare una build in più fra quelle vere.
    if args.iter().any(|a| a == "--install") {
        let tmp_root = DataRoot::new(std::env::temp_dir().join("aethera-e2e-builds"));
        let _ = std::fs::remove_dir_all(&tmp_root.path);
        std::fs::create_dir_all(tmp_root.builds()).map_err(|e| e.to_string())?;
        println!("installo {} · {} in {}", latest.tag, vulkan.backend, tmp_root.path.display());
        let never = AtomicBool::new(false);
        let t = Instant::now();
        let installed = builds::install(
            &tmp_root,
            &latest.tag,
            vulkan,
            probe.free_disk_bytes(&tmp_root.path),
            &never,
            &mut |done, total| {
                if let Some(tot) = total {
                    if done == tot {
                        println!("  scaricati {:.1} MB", done as f64 / 1e6);
                    }
                }
            },
        )?;
        println!("installata in {:.0} s: {installed:?}", t.elapsed().as_secs_f64());
        check(installed.build.as_deref() == Some(latest.tag.as_str()), "--version dice la build che ci si aspettava", &mut failures);
        check(installed.commit.is_some(), "commit letto dal binario installato", &mut failures);
        check(!installed.devices.is_empty(), "--list-devices elenca almeno un dispositivo", &mut failures);
        check(std::path::Path::new(&installed.binary).is_file(), "llama-server presente alla radice della cartella", &mut failures);
        // La build appena installata dev'essere visibile come le altre.
        let m = tmp_root.load_machine().unwrap_or_else(|_| {
            let sys = system::probe().report();
            tmp_root.ensure(&sys).expect("radice temporanea")
        });
        let found = tmp_root.builds_available(&m);
        check(found.iter().any(|b| b.id == installed.id), "la build installata compare fra quelle disponibili", &mut failures);
        // Un digest sbagliato non deve produrre una build installata.
        let mut corrotto = vulkan.clone();
        corrotto.digest = Some(format!("sha256:{}", "0".repeat(64)));
        let second = builds::install(&tmp_root, "b00000", &corrotto, None, &never, &mut |_, _| {});
        check(second.as_ref().is_err_and(|e| e.contains("SHA-256")), "un digest che non coincide blocca l'installazione", &mut failures);
        let _ = std::fs::remove_dir_all(&tmp_root.path);
    } else {
        println!("(installazione vera saltata: ripetere con --install)");
    }

    println!();
    if failures.is_empty() {
        println!("E2E M-04: tutto verde");
        Ok(())
    } else {
        Err(format!("E2E M-04: {} controlli falliti: {failures:?}", failures.len()))
    }
}
