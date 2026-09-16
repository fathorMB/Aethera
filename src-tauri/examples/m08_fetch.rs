//! M-08: scarica pesi e build **con il codice di Aethera** (stessa verifica del Catalogo), da riga
//! di comando, perché le misure della notte non passano dalla finestra.
//!
//! Uso:
//!   `cargo run --release --example m08_fetch -- <radice> model <repo> <file>`
//!   `cargo run --release --example m08_fetch -- <radice> build <tag> <backend>`
//!   `cargo run --release --example m08_fetch -- <radice> releases`
//!
//! I pesi finiscono nella cartella dichiarata in `machine.toml`; le build in `<radice>/builds`.
//! Lo SHA-256 è quello che il publisher dichiara (oid LFS su Hugging Face, digest su GitHub): senza,
//! il file si scarica ma non si dichiara verificato.

use aethera_lib::download::{self, Outcome, Request};
use aethera_lib::machine::DataRoot;
use aethera_lib::{builds, system};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = DataRoot::new(args.first().ok_or("uso: m08_fetch <radice> model|build|releases …")?);
    let probe = system::probe();
    let never = AtomicBool::new(false);
    let t0 = Instant::now();
    let mut last = 0u64;
    let mut progress = move |done: u64, total: Option<u64>| {
        if done.saturating_sub(last) < (512u64 << 20) && done != total.unwrap_or(0) {
            return;
        }
        last = done;
        let s = t0.elapsed().as_secs_f64().max(0.001);
        match total {
            Some(t) => println!(
                "  {:.1}/{:.1} GB ({:.0} %) · {:.1} MB/s",
                done as f64 / 1e9,
                t as f64 / 1e9,
                done as f64 / t as f64 * 100.0,
                done as f64 / 1e6 / s
            ),
            None => println!("  {:.1} GB · {:.1} MB/s", done as f64 / 1e9, done as f64 / 1e6 / s),
        }
    };

    match args.get(1).map(String::as_str) {
        Some("releases") => {
            for r in builds::releases(20)? {
                let vk: Vec<&str> =
                    r.assets.iter().filter(|a| a.backend.contains("vulkan")).map(|a| a.name.as_str()).collect();
                println!("{} {} {:?}", r.tag, r.published.unwrap_or_default(), vk);
            }
        }
        Some("model") => {
            let (repo, file) = (args.get(2).ok_or("repo?")?, args.get(3).ok_or("file?")?);
            let machine = root.load_machine()?;
            let dir = machine.models_dir.ok_or("machine.toml non dichiara la cartella dei pesi")?;
            let remote = download::remote_file(repo, file)?;
            let target = dir.join(file);
            println!(
                "{repo}/{file}\n  {:.2} GB · sha256 {}\n  → {}",
                remote.size.unwrap_or(0) as f64 / 1e9,
                remote.oid.clone().unwrap_or_else(|| "(non dichiarato)".into()),
                target.display()
            );
            let out = download::download(
                &Request {
                    url: remote.url.clone(),
                    target: target.clone(),
                    expected_sha256: remote.oid.clone(),
                    expected_size: remote.size,
                    free_disk: probe.free_disk_bytes(&dir),
                    cancel: &never,
                },
                &mut progress,
            )?;
            match out {
                Outcome::Done { bytes, sha256 } => println!(
                    "FATTO {bytes} byte · sha256 {} · verificato contro l'oid LFS",
                    sha256.unwrap_or_else(|| "(non calcolato)".into())
                ),
                Outcome::AlreadyThere => println!("c'era già: non riscaricato"),
                Outcome::Paused { bytes } => println!("fermato a {bytes} byte"),
            }
        }
        Some("build") => {
            let (tag, backend) = (args.get(2).ok_or("tag?")?, args.get(3).map(String::as_str).unwrap_or("win-vulkan-x64"));
            let release = builds::releases(30)?
                .into_iter()
                .find(|r| r.tag == *tag)
                .ok_or_else(|| format!("{tag} non è fra le release recenti"))?;
            let asset = release
                .assets
                .iter()
                .find(|a| a.backend == backend)
                .ok_or_else(|| format!("{tag} non ha un asset {backend}"))?;
            println!("{} {:.1} MB · digest {:?}", asset.name, asset.size as f64 / 1e6, asset.digest);
            let done = builds::install(&root, tag, asset, probe.free_disk_bytes(&root.builds()), &never, &mut progress)?;
            println!("INSTALLATA {} in {}\n{:?}\n{:?}", done.id, done.dir, done.version_text, done.devices);
            println!("dichiarala in machine.toml con id «{}»", done.id);
        }
        _ => return Err("comando: model | build | releases".into()),
    }
    Ok(())
}
