//! Build di llama.cpp: quali release esistono, quale scaricare e come installarla.
//!
//! Le build ufficiali di `ggml-org/llama.cpp` escono più volte al giorno e sono pubblicate come
//! **prerelease** con tag `b<numero>`: `releases/latest` risponde un'altra cosa e non va usato.
//! Ogni asset porta il proprio `digest` (`sha256:…`), che si verifica **prima** di estrarre.
//!
//! Cambiare build è una variabile di prova, non un aggiornamento automatico: Aethera non
//! aggiorna niente da sola, installa accanto e lascia scegliere.

use crate::download::{self, Request};
use crate::hash;
use crate::machine::{self, DataRoot};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

pub const REPO: &str = "ggml-org/llama.cpp";
const API: &str = "https://api.github.com";
const AGENT: &str = "Aethera (launcher llama-server)";

fn api_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .http_status_as_error(false)
        .user_agent(AGENT)
        .build()
        .into()
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Asset {
    pub name: String,
    pub size: u64,
    /// `sha256:…` pubblicato da GitHub: è la verifica prima dell'estrazione.
    pub digest: Option<String>,
    pub url: String,
    /// `win-vulkan-x64`, `win-cpu-x64`, … dedotto dal nome dell'asset.
    pub backend: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Release {
    /// Tag della build, per esempio `b10989`.
    pub tag: String,
    pub published: Option<String>,
    pub assets: Vec<Asset>,
}

/// Solo i tag di build: `b10989` sì, `v0.4.1` no.
pub fn is_build_tag(tag: &str) -> bool {
    tag.strip_prefix('b').is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

/// `llama-b10989-bin-win-vulkan-x64.zip` → `win-vulkan-x64`.
/// `cudart-llama-…` non è una build del server: resta fuori.
pub fn backend_of(asset: &str) -> Option<String> {
    let rest = asset.strip_prefix("llama-")?.strip_suffix(".zip")?;
    let (_tag, backend) = rest.split_once("-bin-")?;
    (!backend.is_empty()).then(|| backend.to_string())
}

/// Cartella di installazione: `builds/llama-<tag>-<backend>`, come la cerca `machine.rs`.
pub fn dir_name(tag: &str, backend: &str) -> String {
    format!("llama-{tag}-{backend}")
}

/// Le release di build più recenti, dalla più nuova. `limit` è quante ne chiede a GitHub.
pub fn releases(limit: u32) -> Result<Vec<Release>, String> {
    let url = format!("{API}/repos/{REPO}/releases?per_page={}", limit.clamp(1, 100));
    let mut resp = api_agent().get(&url).call().map_err(|e| format!("{url}: {e}"))?;
    let status = resp.status().as_u16();
    if status != 200 {
        return Err(format!("{url}: risposta {status}"));
    }
    let list: serde_json::Value = resp.body_mut().read_json().map_err(|e| format!("{url}: {e}"))?;
    let mut out = Vec::new();
    for r in list.as_array().ok_or_else(|| format!("{url}: risposta inattesa"))? {
        let Some(tag) = r["tag_name"].as_str().filter(|t| is_build_tag(t)) else { continue };
        let assets = r["assets"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| {
                        let name = x["name"].as_str()?.to_string();
                        let backend = backend_of(&name)?;
                        Some(Asset {
                            size: x["size"].as_u64().unwrap_or(0),
                            digest: x["digest"].as_str().map(str::to_string),
                            url: x["browser_download_url"].as_str()?.to_string(),
                            name,
                            backend,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.push(Release { tag: tag.to_string(), published: r["published_at"].as_str().map(str::to_string), assets });
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Installed {
    pub id: String,
    pub dir: String,
    pub binary: String,
    /// `--version` del binario appena installato: build e commit veri, non quelli attesi.
    pub version_text: Option<String>,
    pub build: Option<String>,
    pub commit: Option<String>,
    /// `--list-devices`: che cosa vede questo backend su questa macchina.
    pub devices: Vec<crate::memory::Device>,
}

/// Cerca `llama-server.exe` fino a due livelli sotto: alcune release lo mettono in una sottocartella.
fn find_binary(dir: &Path, depth: u8) -> Option<PathBuf> {
    let name = machine::server_binary_name();
    let direct = dir.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    if depth == 0 {
        return None;
    }
    fs::read_dir(dir).ok()?.flatten().filter(|e| e.path().is_dir()).find_map(|e| find_binary(&e.path(), depth - 1))
}

/// Porta il contenuto di una sottocartella alla radice: `machine.rs` cerca il binario a un livello solo.
fn lift_to_root(root: &Path, inner: &Path) -> Result<(), String> {
    if inner == root {
        return Ok(());
    }
    for e in fs::read_dir(inner).map_err(|e| format!("{}: {e}", inner.display()))?.flatten() {
        let to = root.join(e.file_name());
        fs::rename(e.path(), &to).map_err(|e2| format!("{} → {}: {e2}", e.path().display(), to.display()))?;
    }
    Ok(())
}

fn unzip(archive: &Path, target: &Path) -> Result<(), String> {
    let file = fs::File::open(archive).map_err(|e| format!("{}: {e}", archive.display()))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("{}: {e}", archive.display()))?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| format!("{}: {e}", archive.display()))?;
        // `enclosed_name` rifiuta i percorsi che uscirebbero dalla cartella (zip slip).
        let Some(rel) = entry.enclosed_name() else {
            return Err(format!("{}: contiene un percorso non sicuro ({})", archive.display(), entry.name()));
        };
        let out = target.join(rel);
        if entry.is_dir() {
            fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let mut dest = fs::File::create(&out).map_err(|e| format!("{}: {e}", out.display()))?;
        std::io::copy(&mut entry, &mut dest).map_err(|e| format!("{}: {e}", out.display()))?;
    }
    Ok(())
}

/// Scarica l'asset, **verifica il digest**, estrae in `builds/llama-<tag>-<backend>` e legge
/// `--version` e `--list-devices`. Una build già installata non si tocca.
pub fn install(
    root: &DataRoot,
    tag: &str,
    asset: &Asset,
    free_disk: Option<u64>,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<Installed, String> {
    let name = dir_name(tag, &asset.backend);
    let target = root.builds().join(&name);
    if find_binary(&target, 2).is_some() {
        return Err(format!("{name} è già installata in {}", target.display()));
    }
    fs::create_dir_all(root.builds()).map_err(|e| format!("{}: {e}", root.builds().display()))?;

    let zip_path = root.builds().join(&asset.name);
    let outcome = download::download(
        &Request {
            url: asset.url.clone(),
            target: zip_path.clone(),
            expected_sha256: asset.digest.as_deref().and_then(hash::normalise),
            expected_size: (asset.size > 0).then_some(asset.size),
            free_disk,
            cancel,
        },
        on_progress,
    )?;
    if let download::Outcome::Paused { bytes } = outcome {
        return Err(format!("download interrotto a {bytes} byte: riprendilo per installare {name}"));
    }
    if asset.digest.is_none() {
        // Senza digest pubblicato non si può dire «verificato»: lo si dice, non si finge.
        eprintln!("attenzione: {} non ha un digest pubblicato da GitHub", asset.name);
    }

    fs::create_dir_all(&target).map_err(|e| format!("{}: {e}", target.display()))?;
    unzip(&zip_path, &target)?;
    let binary = find_binary(&target, 2).ok_or_else(|| {
        format!("{} non contiene {}", asset.name, machine::server_binary_name())
    })?;
    if let Some(parent) = binary.parent().filter(|p| *p != target) {
        lift_to_root(&target, &parent.to_path_buf())?;
    }
    let binary = find_binary(&target, 0).ok_or("binario non trovato dopo l'estrazione")?;
    // Lo zip verificato ha già fatto il suo lavoro: tenerlo occuperebbe spazio due volte.
    let _ = fs::remove_file(&zip_path);

    let version = crate::engine::read_version(&binary).ok();
    Ok(Installed {
        id: name.strip_prefix("llama-").unwrap_or(&name).to_string(),
        dir: target.display().to_string(),
        devices: crate::memory::list_devices(&binary).unwrap_or_default(),
        binary: binary.display().to_string(),
        version_text: version.as_ref().map(|v| v.text.clone()),
        build: version.as_ref().and_then(|v| v.build.clone()),
        commit: version.and_then(|v| v.commit),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn only_build_tags_count() {
        assert!(is_build_tag("b10989"));
        // Le build sono prerelease: `latest` dà questo, che non è una build.
        assert!(!is_build_tag("v0.4.1"));
        assert!(!is_build_tag("b"));
        assert!(!is_build_tag("master-b10989"));
    }

    #[test]
    fn the_backend_comes_from_the_asset_name() {
        assert_eq!(backend_of("llama-b10989-bin-win-vulkan-x64.zip").as_deref(), Some("win-vulkan-x64"));
        assert_eq!(backend_of("llama-b10989-bin-win-cpu-x64.zip").as_deref(), Some("win-cpu-x64"));
        assert_eq!(backend_of("llama-b10989-bin-win-rocm-10.0-x64.zip").as_deref(), Some("win-rocm-10.0-x64"));
        // Le librerie CUDA non sono una build del server.
        assert_eq!(backend_of("cudart-llama-bin-win-cuda-12.4-x64.zip"), None);
        // I pacchetti non-zip (ubuntu .tar.gz) restano fuori.
        assert_eq!(backend_of("llama-b10989-bin-ubuntu-x64.tar.gz"), None);
    }

    #[test]
    fn the_install_directory_matches_what_machine_looks_for() {
        let name = dir_name("b10989", "win-vulkan-x64");
        assert_eq!(name, "llama-b10989-win-vulkan-x64");
        // `machine::resolve_build` cerca build e backend fra i pezzi dell'id.
        let id = name.strip_prefix("llama-").unwrap();
        let parts: Vec<&str> = id.split('-').collect();
        assert!(parts.contains(&"b10989") && parts.contains(&"vulkan"));
    }

    #[test]
    fn extracting_lifts_a_nested_layout_to_the_root() {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let tmp = std::env::temp_dir().join(format!("aethera-builds-{n}"));
        fs::create_dir_all(&tmp).unwrap();
        let archive = tmp.join("build.zip");

        // Uno zip con i file dentro una sottocartella, come certe release.
        let mut w = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
        let opts: zip::write::FileOptions<'_, ()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        w.start_file(format!("build/bin/{}", machine::server_binary_name()), opts).unwrap();
        w.write_all(b"finto binario").unwrap();
        w.start_file("build/bin/ggml.dll", opts).unwrap();
        w.write_all(b"finta dll").unwrap();
        w.finish().unwrap();

        let target = tmp.join("llama-b1-win-vulkan-x64");
        fs::create_dir_all(&target).unwrap();
        unzip(&archive, &target).unwrap();
        let found = find_binary(&target, 2).expect("binario trovato in profondità");
        lift_to_root(&target, found.parent().unwrap()).unwrap();
        // Ora sta alla radice, dove `machine::builds_available` lo cerca.
        assert!(target.join(machine::server_binary_name()).is_file());
        assert!(target.join("ggml.dll").is_file());
        let _ = fs::remove_dir_all(&tmp);
    }
}
