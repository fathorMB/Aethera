//! Download riprendibile dei pesi da Hugging Face, con verifica prima dell'uso.
//!
//! Il file arriva dal link diretto (`resolve/main/<file>`), non attraverso un'app terza: si
//! scrive in `<nome>.gguf.part` e diventa `<nome>.gguf` **solo dopo** che lo SHA-256 coincide
//! con l'oid LFS. Un `.part` che resta è una ripresa da fare, non un modello utilizzabile.
//!
//! L'hash atteso si legge dall'API del repository (`/api/models/<repo>/tree/main`), dove ogni
//! file LFS porta `lfs.oid` (è lo SHA-256) e `lfs.size`.

use crate::hash;
use serde::Serialize;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub const HF: &str = "https://huggingface.co";
const AGENT: &str = "Aethera (launcher llama-server)";
const CHUNK: usize = 1 << 20;
/// Margine da lasciare libero sul volume oltre al file da scaricare.
pub const DISK_MARGIN_BYTES: u64 = 2 << 30;

fn agent() -> ureq::Agent {
    // Nessun limite globale: un file da 22 GB dura minuti, ma la connessione deve aprirsi in fretta.
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_global(None)
        .http_status_as_error(false)
        .user_agent(AGENT)
        .build()
        .into()
}

pub fn resolve_url(repo: &str, file: &str) -> String {
    format!("{HF}/{}/resolve/main/{}", repo.trim_matches('/'), file)
}

/// Quello che il publisher dichiara del file: dimensione e oid LFS (lo SHA-256 atteso).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RemoteFile {
    pub repo: String,
    pub file: String,
    pub url: String,
    pub size: Option<u64>,
    pub oid: Option<String>,
}

/// Interroga l'API del repository. Un file non LFS non ha oid: si dice, non si inventa.
pub fn remote_file(repo: &str, file: &str) -> Result<RemoteFile, String> {
    let url = format!("{HF}/api/models/{}/tree/main", repo.trim_matches('/'));
    let mut resp = agent().get(&url).call().map_err(|e| format!("{url}: {e}"))?;
    let status = resp.status().as_u16();
    if status != 200 {
        return Err(format!("{url}: risposta {status} (repository inesistente o privato?)"));
    }
    let list: serde_json::Value = resp.body_mut().read_json().map_err(|e| format!("{url}: {e}"))?;
    let entry = list
        .as_array()
        .ok_or_else(|| format!("{url}: risposta inattesa"))?
        .iter()
        .find(|e| e["path"].as_str() == Some(file))
        .ok_or_else(|| format!("«{file}» non è fra i file di {repo}"))?;
    Ok(RemoteFile {
        repo: repo.to_string(),
        file: file.to_string(),
        url: resolve_url(repo, file),
        size: entry["lfs"]["size"].as_u64().or_else(|| entry["size"].as_u64()),
        oid: entry["lfs"]["oid"].as_str().and_then(hash::normalise),
    })
}

/// Che cosa fare con quello che c'è già sul disco.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    /// Il file finale c'è già: non si riscarica.
    AlreadyThere,
    /// Nessun `.part`: si parte da zero.
    FromScratch,
    /// C'è un `.part` utilizzabile: si riprende da questo byte.
    Resume(u64),
    /// Il `.part` è più grande del file dichiarato: è di un altro file, si ricomincia.
    RestartTooBig(u64),
}

pub fn plan(target: &Path, part_len: Option<u64>, expected_size: Option<u64>) -> Plan {
    if target.is_file() {
        return Plan::AlreadyThere;
    }
    match (part_len, expected_size) {
        (None, _) | (Some(0), _) => Plan::FromScratch,
        (Some(have), Some(total)) if have > total => Plan::RestartTooBig(have),
        (Some(have), _) => Plan::Resume(have),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    /// Scaricato, verificato e rinominato: ora è utilizzabile.
    Done { bytes: u64, sha256: Option<String> },
    /// Il file c'era già.
    AlreadyThere,
    /// Fermato da chi l'ha chiesto: il `.part` resta e si riprende.
    Paused { bytes: u64 },
}

pub struct Request<'a> {
    pub url: String,
    pub target: PathBuf,
    /// SHA-256 atteso (oid LFS): senza, il file si scarica ma non si può dichiarare verificato.
    pub expected_sha256: Option<String>,
    pub expected_size: Option<u64>,
    /// Spazio libero sul volume, se il sistema sa dirlo.
    pub free_disk: Option<u64>,
    pub cancel: &'a AtomicBool,
}

fn part_path(target: &Path) -> PathBuf {
    let mut s = target.as_os_str().to_os_string();
    s.push(crate::catalog::PART_SUFFIX);
    PathBuf::from(s)
}

/// Controlla che il file ci stia, contando quello che è già stato scaricato.
pub fn check_disk(free: Option<u64>, expected_size: Option<u64>, have: u64) -> Result<(), String> {
    let (Some(free), Some(total)) = (free, expected_size) else { return Ok(()) };
    let needed = total.saturating_sub(have).saturating_add(DISK_MARGIN_BYTES);
    if free < needed {
        return Err(format!(
            "spazio insufficiente: servono {:.1} GB (più {:.0} GB di margine), liberi {:.1} GB",
            total.saturating_sub(have) as f64 / 1e9,
            DISK_MARGIN_BYTES as f64 / 1e9,
            free as f64 / 1e9
        ));
    }
    Ok(())
}

/// Scarica, riprendendo se c'è un `.part`. Il file diventa utilizzabile solo dopo la verifica.
pub fn download(req: &Request, on_progress: &mut dyn FnMut(u64, Option<u64>)) -> Result<Outcome, String> {
    let part = part_path(&req.target);
    let part_len = fs::metadata(&part).ok().filter(|m| m.is_file()).map(|m| m.len());
    let mut have = match plan(&req.target, part_len, req.expected_size) {
        Plan::AlreadyThere => return Ok(Outcome::AlreadyThere),
        Plan::FromScratch => 0,
        Plan::Resume(n) => n,
        Plan::RestartTooBig(_) => {
            fs::remove_file(&part).map_err(|e| format!("{}: {e}", part.display()))?;
            0
        }
    };
    check_disk(req.free_disk, req.expected_size, have)?;
    if let Some(dir) = req.target.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }

    let mut call = agent().get(&req.url);
    if have > 0 {
        call = call.header("Range", &format!("bytes={have}-"));
    }
    let mut resp = call.call().map_err(|e| format!("{}: {e}", req.url))?;
    let status = resp.status().as_u16();
    // 206 = ripresa accettata. Con 200 il server manda tutto da capo: si riparte da zero, dicendolo.
    let restart = have > 0 && status == 200;
    if restart {
        have = 0;
    }
    if !(status == 200 || status == 206) {
        return Err(format!("{}: risposta {status}", req.url));
    }
    let total = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .map(|n| n + have)
        .or(req.expected_size);

    let mut file = if have > 0 {
        OpenOptions::new().append(true).open(&part).map_err(|e| format!("{}: {e}", part.display()))?
    } else {
        File::create(&part).map_err(|e| format!("{}: {e}", part.display()))?
    };
    let mut reader = resp.body_mut().as_reader();
    let mut buf = vec![0u8; CHUNK];
    loop {
        if req.cancel.load(Ordering::Relaxed) {
            file.flush().map_err(|e| e.to_string())?;
            return Ok(Outcome::Paused { bytes: have });
        }
        let n = reader.read(&mut buf).map_err(|e| format!("{}: {e}", req.url))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| format!("{}: {e}", part.display()))?;
        have += n as u64;
        on_progress(have, total);
    }
    file.flush().map_err(|e| format!("{}: {e}", part.display()))?;
    drop(file);

    if let Some(total) = req.expected_size {
        if have != total {
            return Ok(Outcome::Paused { bytes: have });
        }
    }

    // Verifica prima di rendere il file utilizzabile: un .gguf con quel nome dev'essere quel file.
    let mut sha = None;
    if let Some(expected) = &req.expected_sha256 {
        match hash::sha256_file(&part, req.cancel, &mut |_, _| {})? {
            hash::Outcome::Cancelled => return Ok(Outcome::Paused { bytes: have }),
            hash::Outcome::Done(got) => {
                if !hash::matches(expected, &got) {
                    // Il .part resta: lo cancella l'operatore, non Aethera di nascosto.
                    return Err(format!(
                        "SHA-256 diverso da quello dichiarato: atteso {}, ottenuto {}. Il file scaricato resta in {}",
                        hash::abbreviate(expected),
                        hash::abbreviate(&got),
                        part.display()
                    ));
                }
                sha = Some(got);
            }
        }
    }
    fs::rename(&part, &req.target).map_err(|e| format!("{} → {}: {e}", part.display(), req.target.display()))?;
    Ok(Outcome::Done { bytes: have, sha256: sha })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_the_direct_link() {
        assert_eq!(
            resolve_url("bartowski/Qwen_Qwen3.6-35B-A3B-GGUF", "Qwen_Qwen3.6-35B-A3B-Q4_K_M.gguf"),
            "https://huggingface.co/bartowski/Qwen_Qwen3.6-35B-A3B-GGUF/resolve/main/Qwen_Qwen3.6-35B-A3B-Q4_K_M.gguf"
        );
    }

    #[test]
    fn plans_the_resume_from_what_is_on_disk() {
        let missing = Path::new("nessun-file-qui.gguf");
        assert_eq!(plan(missing, None, Some(100)), Plan::FromScratch);
        assert_eq!(plan(missing, Some(0), Some(100)), Plan::FromScratch);
        assert_eq!(plan(missing, Some(30), Some(100)), Plan::Resume(30));
        // Un .part più grande del file dichiarato è di un altro file: ricominciare è l'unica scelta onesta.
        assert_eq!(plan(missing, Some(140), Some(100)), Plan::RestartTooBig(140));
        // Senza dimensione dichiarata si riprende comunque da dove si era rimasti.
        assert_eq!(plan(missing, Some(30), None), Plan::Resume(30));
    }

    #[test]
    fn an_existing_file_is_not_downloaded_again() {
        let p = std::env::temp_dir().join("aethera-download-gia-qui.gguf");
        fs::write(&p, b"c'e' gia'").unwrap();
        assert_eq!(plan(&p, None, Some(9)), Plan::AlreadyThere);
        let _ = fs::remove_file(p);
    }

    #[test]
    fn refuses_to_start_without_room_on_disk() {
        let e = check_disk(Some(3 << 30), Some(22_285_080_192), 0).unwrap_err();
        assert!(e.contains("spazio insufficiente"), "{e}");
        // Contando quello che è già stato scaricato, lo stesso spazio può bastare.
        assert!(check_disk(Some(5 << 30), Some(22_285_080_192), 22_000_000_000).is_ok());
        // Se il sistema non sa dire lo spazio libero, non si blocca il download su un'ipotesi.
        assert!(check_disk(None, Some(22_285_080_192), 0).is_ok());
    }
}
