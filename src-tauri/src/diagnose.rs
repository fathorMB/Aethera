//! Da un guasto a una frase che dice **cosa è successo e cosa fare**.
//!
//! Due sorgenti di guasti hanno bisogno di essere tradotte prima di arrivare a chi guarda: il log
//! di `llama-server`, che quando il motore esce con errore contiene la riga che lo spiega in mezzo
//! a centinaia che non c'entrano, e gli errori del sistema operativo, che dicono «errore 112»
//! invece di «il disco è pieno».
//!
//! Qui si riconosce e si suggerisce, non si indovina: una riga che non corrisponde a niente di
//! noto resta com'è, e un codice di uscita senza nessuna riga riconoscibile viene dichiarato tale.
//! Un suggerimento sbagliato manda a cercare nel posto sbagliato, ed è peggio di nessun suggerimento.

use serde::Serialize;

/// Una riga di log che spiega un guasto, con il consiglio quando la riga è riconosciuta.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Reason {
    pub line: String,
    pub hint: Option<String>,
}

/// Parole che in un log di llama.cpp segnalano un guasto, non un avanzamento.
const ALARM: &[&str] = &[
    "error",
    "failed",
    "failure",
    "cannot",
    "unable",
    "not supported",
    "unsupported",
    "unknown argument",
    "unrecognized",
    "invalid",
    "no such file",
    "out of memory",
    "insufficient",
    "assert",
    "abort",
    "terminate called",
    "panic",
    "exception",
    "denied",
];

/// Righe che contengono una parola d'allarme ma raccontano un funzionamento normale.
const NOT_ALARM: &[&str] = &["error_rate", "n_errors = 0", "0 errors", "--no-", "error: 0"];

/// Consigli per i guasti che si ripetono: la riga riconosciuta e che cosa farne.
/// Ogni voce è (parole che devono esserci tutte, consiglio).
const HINTS: &[(&[&str], &str)] = &[
    (
        &["vulkan", "allocation"],
        "la memoria del dispositivo non basta a quello che il profilo chiede: abbassa il contesto, usa una cache KV quantizzata (q8_0) o scegli pesi più piccoli. La stima che la pagina Avvio mostra prima dell'avvio dice quanto serve.",
    ),
    (
        &["outofdevicememory"],
        "la memoria del dispositivo è finita: il modello e la cache al contesto chiesto non ci stanno. Abbassa il contesto, quantizza la cache KV o scegli pesi più piccoli.",
    ),
    (
        &["out of memory"],
        "la memoria non basta a quello che il profilo chiede: abbassa il contesto o quantizza la cache KV. Guarda la stima sulla pagina Avvio e la VRAM libera.",
    ),
    (
        &["invalid argument"],
        "questa build non conosce la leva che il profilo usa: è più vecchia di quello che il profilo chiede, oppure la leva è scritta male in extra_args. Cambia build dal Catalogo o correggi la leva.",
    ),
    (
        &["unknown argument"],
        "questa build non conosce la leva che il profilo usa: è più vecchia di quello che il profilo chiede. Cambia build dal Catalogo o togli la leva.",
    ),
    (
        &["unrecognized"],
        "questa build non conosce la leva che il profilo usa: cambia build dal Catalogo o togli la leva da extra_args.",
    ),
    (
        &["error loading model"],
        "i pesi non si caricano: il file può essere incompleto (verifica lo SHA-256 dal Catalogo) o di un'architettura che questa build non supporta.",
    ),
    (
        &["failed to load model"],
        "i pesi non si caricano: verifica lo SHA-256 dal Catalogo, e se è giusto prova una build più recente.",
    ),
    (
        &["no such file"],
        "un percorso citato dalla riga di comando non esiste più: pesi spostati o cartella rinominata. Il Catalogo dice dove li cerca Aethera.",
    ),
    (
        &["address", "use"],
        "la porta del profilo è già occupata, probabilmente da un altro llama-server: cambia porta nel profilo o ferma l'altro (la pagina Motore riconosce gli orfani).",
    ),
    (
        &["bind"],
        "la porta del profilo non si è potuta aprire: è occupata o l'host dichiarato non esiste su questa macchina.",
    ),
    (
        &["denied"],
        "permessi: Aethera non può leggere o scrivere dove sta provando. Controlla la cartella dei pesi e la radice dati.",
    ),
    (
        &["unsupported"],
        "questa build non supporta quello che il profilo chiede: cambia build dal Catalogo, o togli la leva.",
    ),
];

fn alarming(low: &str) -> bool {
    ALARM.iter().any(|w| low.contains(w)) && !NOT_ALARM.iter().any(|w| low.contains(w))
}

fn hint_for(low: &str) -> Option<String> {
    HINTS.iter().find(|(words, _)| words.iter().all(|w| low.contains(w))).map(|(_, h)| h.to_string())
}

/// Le righe del log che spiegano perché il motore è uscito, dalla più recente all'indietro, senza
/// ripetere due volte la stessa riga. Vuoto vuol dire che il log non dice niente di riconoscibile:
/// allora resta il codice di uscita, e lo si dichiara invece di inventare una causa.
pub fn engine_failure(log: &str, max: usize) -> Vec<Reason> {
    let mut out: Vec<Reason> = Vec::new();
    for line in log.lines().rev() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let low = t.to_ascii_lowercase();
        if !alarming(&low) {
            continue;
        }
        if out.iter().any(|r| r.line == t) {
            continue;
        }
        out.push(Reason { line: t.to_string(), hint: hint_for(&low) });
        if out.len() >= max {
            break;
        }
    }
    out
}

/// Il disco è pieno: Windows lo dice con 39 o 112, i sistemi POSIX con ENOSPC.
pub fn is_disk_full(e: &std::io::Error) -> bool {
    matches!(e.raw_os_error(), Some(39) | Some(112) | Some(28))
}

/// Il volume non c'è più: chiavetta staccata, disco di rete caduto, lettera sparita.
pub fn is_volume_gone(e: &std::io::Error) -> bool {
    // ERROR_NOT_READY(21), ERROR_BAD_NETPATH(53), ERROR_DEV_NOT_EXIST(55), ERROR_INVALID_DRIVE(15)
    matches!(e.raw_os_error(), Some(15) | Some(21) | Some(53) | Some(55))
}

/// Un errore del sistema tradotto in una frase che dice che cosa fare, quando lo si riconosce.
/// Quello che non si riconosce non si traduce: il messaggio originale è già meglio di una perifrasi.
pub fn io_hint(e: &std::io::Error) -> Option<&'static str> {
    use std::io::ErrorKind::*;
    if is_disk_full(e) {
        return Some("il disco è pieno: libera spazio e riprova, quello che è già stato scritto non si perde");
    }
    if is_volume_gone(e) {
        return Some("il disco non risponde: se è esterno o di rete, ricollegalo e riprova");
    }
    match e.kind() {
        PermissionDenied => Some("permessi: Aethera non può scrivere qui. Scegli un'altra cartella o cambia i permessi di questa"),
        NotFound => Some("il percorso non esiste più: è stato spostato o rinominato"),
        _ => None,
    }
}

/// Messaggio completo per un errore di scrittura su un file: che cosa non è riuscito, perché, e
/// che cosa fare. `what` è che cosa si stava facendo, in una parola («scrittura», «copia»).
pub fn write_error(what: &str, path: &std::path::Path, e: &std::io::Error) -> String {
    match io_hint(e) {
        Some(h) => format!("{what} di {} non riuscita: {e} — {h}", path.display()),
        None => format!("{what} di {} non riuscita: {e}", path.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un log vero di llama-server che non riesce ad allocare sulla GPU.
    const VULKAN_OOM: &str = r#"
load_tensors: offloading 41 repeating layers to GPU
load_tensors: offloaded 42/42 layers to GPU
llama_kv_cache: Vulkan0 KV buffer size = 10240.00 MiB
ggml_vulkan: Device memory allocation of size 2147483648 failed
ggml_vulkan: vk::Device::allocateMemory: ErrorOutOfDeviceMemory
llama_init_from_model: failed to initialize the context
common_init_from_params: failed to create context with model 'C:\pesi\M.gguf'
"#;

    #[test]
    fn picks_the_lines_that_explain_the_exit_newest_first() {
        let r = engine_failure(VULKAN_OOM, 3);
        assert_eq!(r.len(), 3);
        assert!(r[0].line.starts_with("common_init_from_params"), "{r:?}");
        assert!(r.iter().any(|x| x.line.contains("allocateMemory")));
        // Le righe generiche restano com'erano; quella riconosciuta porta il consiglio.
        assert!(r[0].hint.is_none(), "«failed to create context» non dice da solo che cosa fare");
        let vram = r.iter().find(|x| x.hint.is_some()).expect("almeno una riga riconosciuta");
        assert!(vram.line.contains("OutOfDeviceMemory"), "{r:?}");
        assert!(vram.hint.as_deref().unwrap().contains("contesto"));
    }

    #[test]
    fn a_vulkan_allocation_failure_says_what_to_lower() {
        let r = engine_failure("ggml_vulkan: Device memory allocation of size 2147483648 failed", 5);
        assert_eq!(r.len(), 1);
        assert!(r[0].hint.as_deref().unwrap().contains("contesto"), "{r:?}");
    }

    #[test]
    fn a_lever_the_build_does_not_know_says_it_is_the_build() {
        // La forma vera con cui b10809 rifiuta una leva che non conosce.
        let r = engine_failure("error: invalid argument: --questa-leva-non-esiste", 5);
        assert_eq!(r.len(), 1);
        assert!(r[0].hint.as_deref().unwrap().contains("build"), "{r:?}");
    }

    #[test]
    fn an_old_build_that_does_not_know_a_lever_says_so() {
        let r = engine_failure("error: invalid argument: --spec-draft-n-max\nerror while handling argument \"--spec-draft-n-max\": unknown argument", 5);
        assert!(r.iter().any(|x| x.hint.as_deref().is_some_and(|h| h.contains("build"))), "{r:?}");
    }

    #[test]
    fn a_log_that_explains_nothing_produces_nothing() {
        let quiet = "main: server is listening on http://127.0.0.1:8080\nsrv  update_slots: all slots are idle\n";
        assert!(engine_failure(quiet, 5).is_empty(), "meglio il codice di uscita di una causa inventata");
    }

    #[test]
    fn normal_lines_that_contain_an_alarm_word_are_not_failures() {
        assert!(engine_failure("srv  log_server_r: error_rate = 0.0\n", 5).is_empty());
        assert!(engine_failure("main: --no-warmup\n", 5).is_empty());
    }

    #[test]
    fn the_same_line_repeated_is_reported_once() {
        let log = "ggml_vulkan: allocation failed\nggml_vulkan: allocation failed\nggml_vulkan: allocation failed\n";
        assert_eq!(engine_failure(log, 5).len(), 1);
    }

    #[test]
    fn a_full_disk_is_said_in_words() {
        let full = std::io::Error::from_raw_os_error(112);
        assert!(is_disk_full(&full));
        assert!(io_hint(&full).unwrap().contains("disco è pieno"));
        let denied = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "accesso negato");
        assert!(io_hint(&denied).unwrap().contains("permessi"));
        // Quello che non si riconosce non si traduce.
        assert_eq!(io_hint(&std::io::Error::other("qualcosa")), None);
        let msg = write_error("scrittura", std::path::Path::new(r"D:\pesi\M.gguf.part"), &full);
        assert!(msg.contains(r"D:\pesi\M.gguf.part") && msg.contains("disco è pieno"), "{msg}");
    }

    #[test]
    fn a_disconnected_volume_is_not_confused_with_a_full_one() {
        let gone = std::io::Error::from_raw_os_error(21);
        assert!(is_volume_gone(&gone) && !is_disk_full(&gone));
        assert!(io_hint(&gone).unwrap().contains("ricollegalo"));
    }
}
