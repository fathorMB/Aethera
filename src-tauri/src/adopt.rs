//! Registrare pesi che stanno **fuori** dalla cartella dichiarata, senza duplicarli.
//!
//! I profili citano i pesi per nome file dentro la cartella della macchina: un `.gguf` che sta
//! altrove deve avere un nome lì dentro. Avere un nome non vuol dire essere copiato: sullo stesso
//! volume un hard link è un secondo nome per lo stesso contenuto e non occupa un altro byte. Su un
//! volume diverso l'hard link non esiste e l'unica via è copiare: lo si dice prima, con quanto
//! spazio serve, e lo decide chi guarda — non lo si fa di nascosto.
//!
//! Il riconoscimento di «ce l'ho già» non passa dal nome: due nomi con la stessa identità di file
//! (volume + indice) sono lo stesso file, anche se uno l'ha creato LM Studio con `lms import -L`.

use crate::catalog::PART_SUFFIX;
use crate::system::SystemProbe;
use serde::Serialize;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Margine da lasciare libero sul volume oltre ai byte da copiare.
pub const DISK_MARGIN_BYTES: u64 = 2 << 30;
const CHUNK: usize = 8 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Il file è già nella cartella, con questo nome o con un altro: basta riscansionare.
    Nothing,
    /// Stesso volume: un secondo nome per lo stesso contenuto, zero byte in più.
    Link,
    /// Volume diverso: i byte vengono occupati due volte. Richiede una conferma.
    Copy,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Plan {
    pub source: String,
    /// Nome che il file avrà nella cartella dei pesi: è quello che citerà il profilo.
    pub file: String,
    pub target: String,
    pub bytes: u64,
    pub action: Action,
    /// Il file già presente che è lo stesso file, quando c'è.
    pub already: Option<String>,
    /// Quello che chi guarda deve sapere prima di confermare.
    pub notes: Vec<String>,
    /// Perché non si può fare: pieno significa che nessuna azione è possibile così com'è.
    pub blocker: Option<String>,
    pub free_disk: Option<u64>,
}

/// Radice del volume di un percorso assoluto (`C:\`, `\\server\share\`): due percorsi con la
/// stessa radice stanno sullo stesso volume e quindi possono essere collegati.
fn volume_of(path: &Path) -> Option<String> {
    let s = path.to_string_lossy().replace('/', "\\");
    if let Some(rest) = s.strip_prefix("\\\\") {
        // UNC: \\server\share
        let mut it = rest.splitn(3, '\\');
        let (server, share) = (it.next()?, it.next()?);
        if server.is_empty() || share.is_empty() {
            return None;
        }
        return Some(format!("\\\\{}\\{}", server.to_ascii_lowercase(), share.to_ascii_lowercase()));
    }
    let bytes = s.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' {
        return Some(s[..2].to_ascii_lowercase());
    }
    // Su un sistema senza lettere di volume la domanda non si pone: lo dichiariamo sconosciuto.
    None
}

/// Il file già presente nella cartella che è **lo stesso file** del sorgente (hard link).
fn linked_twin(source: &Path, dir: &Path, probe: &dyn SystemProbe) -> Option<String> {
    let id = probe.file_id(source)?;
    let entries = fs::read_dir(dir).ok()?;
    for e in entries.flatten() {
        let p = e.path();
        if !p.is_file() || !p.to_string_lossy().to_ascii_lowercase().ends_with(".gguf") {
            continue;
        }
        if probe.file_id(&p) == Some(id) {
            return Some(e.file_name().to_string_lossy().to_string());
        }
    }
    None
}

/// Che cosa comporta registrare `source` nella cartella dei pesi. Non tocca niente.
pub fn plan(source: &Path, models_dir: &Path, probe: &dyn SystemProbe) -> Result<Plan, String> {
    let md = fs::metadata(source).map_err(|e| format!("{}: {e}", source.display()))?;
    if !md.is_file() {
        return Err(format!("{} non è un file", source.display()));
    }
    let file = source.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    if !file.to_ascii_lowercase().ends_with(".gguf") {
        return Err(format!("«{file}» non è un .gguf: la cartella dei pesi contiene modelli, non altro"));
    }
    if !models_dir.is_dir() {
        return Err(format!("cartella dei pesi non trovata: {}", models_dir.display()));
    }
    let target = models_dir.join(&file);
    let mut p = Plan {
        source: source.display().to_string(),
        file: file.clone(),
        target: target.display().to_string(),
        bytes: md.len(),
        action: Action::Link,
        already: None,
        notes: Vec::new(),
        blocker: None,
        free_disk: None,
    };

    // 1. Sta già dentro: non c'è niente da collegare, il catalogo lo trova da solo.
    let same_dir = match (fs::canonicalize(source.parent().unwrap_or(models_dir)), fs::canonicalize(models_dir)) {
        (Ok(a), Ok(b)) => a == b,
        _ => source.parent() == Some(models_dir),
    };
    if same_dir {
        p.action = Action::Nothing;
        p.already = Some(file);
        p.notes.push("è già nella cartella dei pesi: il catalogo lo trova riscansionando".into());
        return Ok(p);
    }

    // 2. Lo stesso file c'è già lì dentro con un altro nome: è un hard link, non un doppione.
    if let Some(twin) = linked_twin(source, models_dir, probe) {
        p.action = Action::Nothing;
        p.already = Some(twin.clone());
        p.notes.push(format!("è lo stesso file di «{twin}», già nella cartella: un altro strumento l'ha collegato lì"));
        return Ok(p);
    }

    // 3. Quel nome è occupato da un file diverso: non si sovrascrive niente.
    if target.exists() {
        p.action = Action::Nothing;
        p.blocker = Some(format!(
            "nella cartella dei pesi c'è già un file «{file}», e non è questo: rinomina uno dei due prima di registrarlo"
        ));
        return Ok(p);
    }

    // 4. Stesso volume: hard link. Volume diverso: solo copia, dichiarata.
    let (vs, vt) = (volume_of(source), volume_of(models_dir));
    if vs.is_some() && vs == vt {
        p.notes.push("stesso volume: un hard link dà un secondo nome allo stesso contenuto, senza occupare altro spazio".into());
        return Ok(p);
    }
    p.action = Action::Copy;
    p.free_disk = probe.free_disk_bytes(models_dir);
    p.notes.push(format!(
        "il file sta su un altro volume ({} contro {}): un hard link non è possibile e copiarlo occupa gli stessi byte due volte",
        vs.as_deref().unwrap_or("sconosciuto"),
        vt.as_deref().unwrap_or("sconosciuto")
    ));
    if let Some(free) = p.free_disk {
        if free < md.len() + DISK_MARGIN_BYTES {
            p.blocker = Some(format!(
                "spazio insufficiente: servono {:.1} GB più {:.0} GB di margine, liberi {:.1} GB",
                md.len() as f64 / 1e9,
                DISK_MARGIN_BYTES as f64 / 1e9,
                free as f64 / 1e9
            ));
        }
    }
    Ok(p)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    /// Non c'era niente da fare: il file era già registrabile così com'è.
    AlreadyThere { file: String },
    Linked { file: String },
    Copied { file: String, bytes: u64 },
    /// Fermata da chi l'ha chiesta: il `.part` resta e non sporca la cartella con un file a metà.
    Cancelled { bytes: u64 },
}

/// Esegue il piano. `Action::Copy` si esegue solo se chi chiama l'ha confermata.
pub fn apply(
    plan: &Plan,
    confirm_copy: bool,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(u64, u64),
) -> Result<Outcome, String> {
    if let Some(b) = &plan.blocker {
        return Err(b.clone());
    }
    let source = PathBuf::from(&plan.source);
    let target = PathBuf::from(&plan.target);
    match plan.action {
        Action::Nothing => Ok(Outcome::AlreadyThere { file: plan.already.clone().unwrap_or_else(|| plan.file.clone()) }),
        Action::Link => {
            fs::hard_link(&source, &target).map_err(|e| {
                format!(
                    "collegamento non riuscito ({} → {}): {e}. Se i due percorsi non sono sullo stesso volume resta la copia.",
                    source.display(),
                    target.display()
                )
            })?;
            Ok(Outcome::Linked { file: plan.file.clone() })
        }
        Action::Copy => {
            if !confirm_copy {
                return Err("la copia occupa i byte due volte: serve una conferma esplicita".into());
            }
            copy_with_progress(&source, &target, plan.bytes, cancel, on_progress)
        }
    }
}

/// Copia in `<nome>.part` e rinomina alla fine: una copia interrotta non diventa mai un modello.
fn copy_with_progress(
    source: &Path,
    target: &Path,
    total: u64,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(u64, u64),
) -> Result<Outcome, String> {
    let part = PathBuf::from(format!("{}{PART_SUFFIX}", target.display()));
    let mut input = fs::File::open(source).map_err(|e| format!("{}: {e}", source.display()))?;
    let mut output = fs::File::create(&part).map_err(|e| format!("{}: {e}", part.display()))?;
    let mut buf = vec![0u8; CHUNK];
    let mut done: u64 = 0;
    loop {
        if cancel.load(Ordering::Relaxed) {
            drop(output);
            let _ = fs::remove_file(&part);
            return Ok(Outcome::Cancelled { bytes: done });
        }
        let n = input.read(&mut buf).map_err(|e| format!("{}: {e}", source.display()))?;
        if n == 0 {
            break;
        }
        output.write_all(&buf[..n]).map_err(|e| format!("{}: {e}", part.display()))?;
        done += n as u64;
        on_progress(done, total);
    }
    output.flush().map_err(|e| format!("{}: {e}", part.display()))?;
    drop(output);
    fs::rename(&part, target).map_err(|e| format!("{} → {}: {e}", part.display(), target.display()))?;
    Ok(Outcome::Copied { file: target.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), bytes: done })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::UnknownProbe;

    fn tmp(tag: &str) -> PathBuf {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let d = std::env::temp_dir().join(format!("aethera-adopt-{tag}-{n}"));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn volumes_are_compared_without_caring_about_case_or_slashes() {
        assert_eq!(volume_of(Path::new(r"C:\Git\pesi\x.gguf")).as_deref(), Some("c:"));
        assert_eq!(volume_of(Path::new("D:/pesi")).as_deref(), Some("d:"));
        assert_eq!(volume_of(Path::new(r"\\NAS\Modelli\x.gguf")).as_deref(), Some(r"\\nas\modelli"));
        assert_eq!(volume_of(Path::new("relativo/x.gguf")), None);
    }

    #[test]
    fn a_file_already_in_the_weights_folder_needs_nothing() {
        let d = tmp("inside");
        let models = d.join("pesi");
        fs::create_dir_all(&models).unwrap();
        let f = models.join("M.gguf");
        fs::write(&f, b"pesi").unwrap();
        let p = plan(&f, &models, &UnknownProbe).unwrap();
        assert_eq!(p.action, Action::Nothing);
        assert_eq!(p.already.as_deref(), Some("M.gguf"));
        assert_eq!(apply(&p, false, &AtomicBool::new(false), &mut |_, _| {}).unwrap(), Outcome::AlreadyThere { file: "M.gguf".into() });
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_file_outside_is_linked_not_copied_and_the_name_is_never_overwritten() {
        let d = tmp("link");
        let (models, altrove) = (d.join("pesi"), d.join("altrove"));
        fs::create_dir_all(&models).unwrap();
        fs::create_dir_all(&altrove).unwrap();
        let src = altrove.join("M.gguf");
        fs::write(&src, b"contenuto dei pesi").unwrap();

        let p = plan(&src, &models, &UnknownProbe).unwrap();
        assert_eq!(p.action, Action::Link, "{p:?}");
        assert_eq!(p.bytes, 18);
        assert!(p.blocker.is_none());
        assert_eq!(apply(&p, false, &AtomicBool::new(false), &mut |_, _| {}).unwrap(), Outcome::Linked { file: "M.gguf".into() });
        assert_eq!(fs::read(models.join("M.gguf")).unwrap(), b"contenuto dei pesi");

        // Rifarlo non sovrascrive: ora quel nome è occupato…
        let p2 = plan(&src, &models, &UnknownProbe).unwrap();
        // …e su Windows la sonda vera riconosce che è lo stesso file, quindi non c'è niente da fare.
        #[cfg(windows)]
        {
            let probe = crate::system::probe();
            let p3 = plan(&src, &models, probe.as_ref()).unwrap();
            assert_eq!(p3.action, Action::Nothing);
            assert_eq!(p3.already.as_deref(), Some("M.gguf"));
            assert_eq!(probe.hard_links(&src), Some(2), "il collegamento non ha duplicato i byte");
        }
        // Senza sonda l'identità non è nota: allora si rifiuta il nome occupato invece di sovrascriverlo.
        assert!(p2.blocker.is_some());
        assert!(apply(&p2, true, &AtomicBool::new(false), &mut |_, _| {}).is_err());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_copy_leaves_no_half_file_when_it_is_stopped() {
        let d = tmp("copia");
        let (models, altrove) = (d.join("pesi"), d.join("altrove"));
        fs::create_dir_all(&models).unwrap();
        fs::create_dir_all(&altrove).unwrap();
        let src = altrove.join("M.gguf");
        fs::write(&src, vec![7u8; 32 << 20]).unwrap();
        let mut p = plan(&src, &models, &UnknownProbe).unwrap();
        p.action = Action::Copy; // stesso volume nel test: forziamo la via della copia

        // Senza conferma non parte: i byte doppi sono una scelta di chi guarda.
        assert!(apply(&p, false, &AtomicBool::new(false), &mut |_, _| {}).is_err());

        let cancel = AtomicBool::new(false);
        let out = apply(&p, true, &cancel, &mut |done, _| {
            if done > 0 {
                cancel.store(true, Ordering::Relaxed);
            }
        })
        .unwrap();
        assert!(matches!(out, Outcome::Cancelled { .. }), "{out:?}");
        assert!(!models.join("M.gguf").exists(), "una copia fermata non diventa un modello");
        assert!(!models.join(format!("M.gguf{PART_SUFFIX}")).exists(), "il .part di una copia fermata non resta");

        let out = apply(&p, true, &AtomicBool::new(false), &mut |_, _| {}).unwrap();
        assert_eq!(out, Outcome::Copied { file: "M.gguf".into(), bytes: 32 << 20 });
        assert_eq!(fs::metadata(models.join("M.gguf")).unwrap().len(), 32 << 20);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn only_gguf_files_enter_the_weights_folder() {
        let d = tmp("estensione");
        fs::create_dir_all(d.join("pesi")).unwrap();
        let src = d.join("note.txt");
        fs::write(&src, b"x").unwrap();
        assert!(plan(&src, &d.join("pesi"), &UnknownProbe).unwrap_err().contains(".gguf"));
        let _ = fs::remove_dir_all(&d);
    }
}
