//! SHA-256 di un file grande, con avanzamento e annullamento.
//!
//! Su 22 GB il calcolo sono minuti: non si fa all'avvio né a ogni giro della finestra, ma solo
//! quando l'operatore lo chiede o quando un download finisce. Chi lo lancia può fermarlo.
//!
//! L'hash atteso viene da fuori (l'oid LFS di Hugging Face, il `digest` di una release GitHub):
//! qui si calcola soltanto, e il confronto è esplicito.

use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// Blocco di lettura: grande abbastanza da non pagare il costo per chiamata su file da decine di GB.
const CHUNK: usize = 8 << 20;

/// Esito del calcolo: `Cancelled` non è un errore, è una scelta di chi l'ha chiesto.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Done(String),
    Cancelled,
}

/// Calcola lo SHA-256 leggendo il file a blocchi. `on_progress` riceve (byte letti, byte totali).
pub fn sha256_file(
    path: &Path,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(u64, u64),
) -> Result<Outcome, String> {
    let mut f = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let total = f.metadata().map_err(|e| format!("{}: {e}", path.display()))?.len();
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    let mut done: u64 = 0;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(Outcome::Cancelled);
        }
        let n = f.read(&mut buf).map_err(|e| format!("{}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        done += n as u64;
        on_progress(done, total);
    }
    Ok(Outcome::Done(hex(&hasher.finalize())))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Minuscole **prima** di togliere il prefisso: GitHub può scrivere `SHA256:` in maiuscolo.
fn clean(raw: &str) -> String {
    raw.trim().to_ascii_lowercase().trim_start_matches("sha256:").trim().to_string()
}

/// Due hash sono lo stesso hash a prescindere da maiuscole e da un eventuale prefisso `sha256:`
/// (la API di GitHub scrive il digest così).
pub fn matches(expected: &str, actual: &str) -> bool {
    let (a, b) = (clean(expected), clean(actual));
    !a.is_empty() && a == b
}

/// Un SHA-256 valido: 64 cifre esadecimali, eventualmente con il prefisso `sha256:`.
pub fn normalise(raw: &str) -> Option<String> {
    let s = clean(raw);
    (s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())).then_some(s)
}

/// Forma corta per la finestra: `b46fedd3…9064772`.
pub fn abbreviate(hash: &str) -> String {
    match normalise(hash) {
        Some(h) => format!("{}…{}", &h[..8], &h[57..]),
        None => hash.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_file(tag: &str, bytes: &[u8]) -> std::path::PathBuf {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let p = std::env::temp_dir().join(format!("aethera-hash-{tag}-{n}"));
        File::create(&p).unwrap().write_all(bytes).unwrap();
        p
    }

    #[test]
    fn hashes_a_file_and_reports_progress() {
        let p = temp_file("abc", b"abc");
        let mut seen = Vec::new();
        let out = sha256_file(&p, &AtomicBool::new(false), &mut |d, t| seen.push((d, t))).unwrap();
        assert_eq!(out, Outcome::Done("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into()));
        assert_eq!(seen, vec![(3, 3)]);
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn cancelling_stops_without_an_error() {
        let p = temp_file("annulla", &vec![7u8; 4096]);
        let out = sha256_file(&p, &AtomicBool::new(true), &mut |_, _| {}).unwrap();
        assert_eq!(out, Outcome::Cancelled);
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn a_missing_file_is_a_readable_error() {
        let e = sha256_file(Path::new("nessun-file-qui.gguf"), &AtomicBool::new(false), &mut |_, _| {}).unwrap_err();
        assert!(e.contains("nessun-file-qui.gguf"), "{e}");
    }

    #[test]
    fn comparison_ignores_case_and_the_github_prefix() {
        let g1 = "b46fedd33e0bfb0cae308aa3c158d0a4b2c4a1d2185a1ed6f093cdaf39064772";
        assert!(matches(g1, &g1.to_ascii_uppercase()));
        assert!(matches(&format!("sha256:{g1}"), g1), "il digest di GitHub arriva con il prefisso");
        assert!(matches(&format!("SHA256:{}", g1.to_ascii_uppercase()), g1), "prefisso e cifre in maiuscolo");
        assert!(!matches(g1, "b46fedd3"));
        // Un hash atteso vuoto non «coincide» con niente.
        assert!(!matches("", ""));
    }

    #[test]
    fn abbreviation_and_validation() {
        let g1 = "b46fedd33e0bfb0cae308aa3c158d0a4b2c4a1d2185a1ed6f093cdaf39064772";
        assert_eq!(abbreviate(g1), "b46fedd3…9064772");
        assert_eq!(normalise(&format!("  SHA256:{}  ", g1.to_ascii_uppercase())).as_deref(), Some(g1));
        assert_eq!(normalise("troppo corto"), None);
        // Quello che non è un hash si mostra com'è, senza fingere di accorciarlo.
        assert_eq!(abbreviate("oid ignoto"), "oid ignoto");
    }
}
