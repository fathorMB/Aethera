//! Avvio, arresto e sorveglianza dei motori di servizio (M-20 T-06).
//!
//! È il gemello piccolo di [`crate::engine`], e le differenze sono tutte volute:
//!
//! - **nessun manifest, nessuna telemetria, nessuno storico.** Un servizio non è il soggetto di una
//!   misura: `runs/` resta la casa del motore principale. Quello che di un servizio si vuole sapere
//!   — risponde? quanto occupa? da quanto è su? — si legge adesso, non si confronta con ieri;
//! - **nessuno stato «in uso».** Un servizio non rende il motore principale occupato e non blocca
//!   arresto né riavvio. Se un embedding notturno impedisse all'operatore di riavviare il motore,
//!   avremmo scambiato il servo col padrone;
//! - **più di uno alla volta**, ognuno sulla sua porta. Il motore principale resta uno solo.
//!
//! La VRAM di ogni servizio si legge **per processo** (`\GPU Process Memory(pid_N_*)`), la stessa
//! strada che il motore principale usa già: è la sua memoria, non una differenza fra due totali.
//!
//! La precedenza al coding non sta qui: Aethera dice com'è messa (`in_use` e i lock su `/status`),
//! e chi usa i servizi decide se fermarsi. Sospendere d'autorità un processo che sta rispondendo
//! farebbe scadere le richieste in volo e metterebbe in Aethera una politica che non è sua.

use crate::job::Job;
use crate::service::{self, ServiceProfile};
use crate::system::SystemProbe;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Un servizio piccolo carica in pochi secondi; oltre questo tempo qualcosa non va e va detto.
const READY_TIMEOUT: Duration = Duration::from_secs(180);
const POLL: Duration = Duration::from_millis(500);

pub struct StartService {
    pub profile: ServiceProfile,
    /// Percorso del binario `llama-server` della build che il profilo dichiara.
    pub binary: PathBuf,
    pub model: PathBuf,
}

struct Running {
    profile: ServiceProfile,
    child: Child,
    job: Job,
    pid: u32,
    command_line: String,
    started_at: String,
    ready_ms: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ServiceView {
    pub name: String,
    /// `embedding`, `rerank` o `chat`.
    pub kind: String,
    pub base_url: String,
    pub port: u16,
    pub model: String,
    pub pid: u32,
    pub started_at: String,
    pub ready_ms: u64,
    pub command_line: String,
    /// `ready`, `loading` o `exited`.
    pub state: String,
    /// VRAM dedicata di **questo** processo, misurata. `None` quando il contatore non risponde.
    pub vram_dedicated_gib: Option<f64>,
    /// Vero solo dopo un controllo riuscito che non sia `/health`: per il reranker è il test di
    /// sanità, perché un GGUF convertito male risponde 200 e poi dà punteggi vicini a zero.
    pub sane: Option<bool>,
    /// Dimensioni dichiarate dal profilo, per i servizi di embedding. Chi consuma i vettori le
    /// fissa in configurazione: vanno riportate, non indovinate.
    pub embed_dim: Option<u32>,
}

#[derive(Default)]
struct State {
    running: BTreeMap<String, Running>,
    /// Esito dell'ultimo controllo di sanità, per nome del servizio.
    sane: BTreeMap<String, bool>,
}

#[derive(Clone, Default)]
pub struct Services {
    state: Arc<Mutex<State>>,
}

fn now() -> String {
    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).build().into()
}

impl Services {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn is_running(&self, name: &str) -> bool {
        self.lock().running.contains_key(name)
    }

    pub fn names(&self) -> Vec<String> {
        self.lock().running.keys().cloned().collect()
    }

    /// Avvia un servizio e aspetta che risponda. Il nome è la chiave: due servizi con lo stesso
    /// nome non possono convivere, come per i profili.
    pub fn start(&self, req: StartService) -> Result<ServiceView, String> {
        let name = req.profile.name.clone();
        if self.is_running(&name) {
            return Err(format!("il servizio «{name}» è già acceso"));
        }
        if !req.binary.exists() {
            return Err(format!("binario non trovato: {}", req.binary.display()));
        }
        if !req.model.exists() {
            return Err(format!("pesi non trovati: {}", req.model.display()));
        }
        let port = req.profile.server.port;
        if port_busy(port) {
            return Err(format!(
                "la porta {port} è occupata: un altro processo la sta usando, forse un servizio non avviato da Aethera"
            ));
        }

        let args = service::build_args(&req.profile, &req.model);
        let command_line = format!("\"{}\" {}", req.binary.display(), args.join(" "));

        let mut cmd = Command::new(&req.binary);
        cmd.args(&args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let mut child = cmd.spawn().map_err(|e| format!("{}: {e}", req.binary.display()))?;

        let job = Job::kill_on_close()?;
        if let Err(e) = job.assign(&child) {
            let _ = child.kill();
            return Err(e);
        }
        let pid = child.id();

        // Aspetta «pronto». Se il processo esce prima, il motivo è il suo codice d'uscita: si dice
        // quello, invece di lasciare scadere il timeout e raccontare una bugia sul tempo.
        let base = service::base_url(&req.profile);
        let http = agent();
        let t0 = Instant::now();
        loop {
            if let Ok(Some(code)) = child.try_wait() {
                return Err(format!("il servizio «{name}» è uscito durante il caricamento (codice {code})"));
            }
            // `ureq::Error` non implementa PartialEq: si confronta il numero, non il Result.
            if http.get(&format!("{base}/health")).call().map(|r| r.status().as_u16()).unwrap_or(0) == 200 {
                break;
            }
            if t0.elapsed() > READY_TIMEOUT {
                let _ = child.kill();
                job.terminate();
                return Err(format!(
                    "il servizio «{name}» non è arrivato a pronto in {} s",
                    READY_TIMEOUT.as_secs()
                ));
            }
            std::thread::sleep(POLL);
        }
        let ready_ms = t0.elapsed().as_millis() as u64;

        let running = Running {
            profile: req.profile,
            child,
            job,
            pid,
            command_line,
            started_at: now(),
            ready_ms,
        };
        let view = view_of(&running, None, None);
        self.lock().running.insert(name, running);
        Ok(view)
    }

    /// Ferma un servizio. Non aspetta che sia «libero»: un servizio non ha uno stato «in uso», e
    /// una richiesta in volo dura millisecondi.
    pub fn stop(&self, name: &str) -> Result<(), String> {
        let mut st = self.lock();
        let mut r = st.running.remove(name).ok_or_else(|| format!("il servizio «{name}» non è acceso"))?;
        st.sane.remove(name);
        drop(st);
        let _ = r.child.kill();
        let _ = r.child.wait();
        r.job.terminate();
        Ok(())
    }

    pub fn stop_all(&self) {
        for name in self.names() {
            let _ = self.stop(&name);
        }
    }

    /// Lo stato di tutti i servizi, con la VRAM letta adesso dal contatore per processo.
    pub fn view(&self, probe: &dyn SystemProbe) -> Vec<ServiceView> {
        let mut st = self.lock();
        let sane = st.sane.clone();
        let mut out = Vec::new();
        let mut morti = Vec::new();
        for (name, r) in st.running.iter_mut() {
            let uscito = matches!(r.child.try_wait(), Ok(Some(_)));
            let vram = if uscito { None } else { probe.process_memory(r.pid).gpu_dedicated_gib };
            let stato = if uscito { "exited" } else { "ready" };
            out.push(ServiceView { state: stato.into(), ..view_of(r, vram, sane.get(name).copied()) });
            if uscito {
                morti.push(name.clone());
            }
        }
        // Un servizio uscito da solo resta visibile una volta con `exited`, poi sparisce: la
        // finestra deve poter dire «è caduto», non far finta che non sia mai esistito.
        for name in morti {
            st.running.remove(&name);
        }
        out
    }

    /// Registra l'esito di un controllo di sanità fatto da fuori (per il reranker, dove `/health`
    /// verde non basta: un GGUF convertito male risponde e sbaglia i punteggi).
    pub fn set_sane(&self, name: &str, sane: bool) {
        self.lock().sane.insert(name.to_string(), sane);
    }

    /// Toglie «uccidi alla chiusura» a tutti: i servizi sopravvivono all'uscita di Aethera, come
    /// può fare il motore principale.
    pub fn release_all(&self) {
        for r in self.lock().running.values() {
            r.job.release();
        }
    }
}

fn view_of(r: &Running, vram: Option<f64>, sane: Option<bool>) -> ServiceView {
    ServiceView {
        name: r.profile.name.clone(),
        kind: r.profile.service.kind.clone(),
        base_url: service::base_url(&r.profile),
        port: r.profile.server.port,
        model: r.profile.model.file.clone(),
        pid: r.pid,
        started_at: r.started_at.clone(),
        ready_ms: r.ready_ms,
        command_line: r.command_line.clone(),
        state: "ready".into(),
        vram_dedicated_gib: vram,
        sane,
        embed_dim: r.profile.service.embed_dim,
    }
}

fn port_busy(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_millis(300),
    )
    .is_ok()
}

/// Il test di sanità del reranker: un documento pertinente e due che non c'entrano. Un GGUF
/// convertito male risponde 200 e dà a tutti punteggi vicini a zero, e questo lo vede.
/// Restituisce il punteggio del documento pertinente, che chi chiama confronta con una soglia.
pub fn rerank_sanity(base_url: &str) -> Result<f64, String> {
    let body = serde_json::json!({
        "query": "Quanta memoria occupa il motore dopo il caricamento?",
        "documents": [
            "La ricetta della carbonara vuole guanciale, uovo e pecorino.",
            "Dopo il caricamento Aethera misura la VRAM dedicata e la RAM rimasta.",
            "Il treno per Milano parte dal binario nove alle sette e mezza."
        ]
    });
    let text = agent()
        .post(&format!("{base_url}/v1/rerank"))
        .send_json(&body)
        .map_err(|e| format!("/v1/rerank: {e}"))?
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let results = v.get("results").and_then(|r| r.as_array()).ok_or("risposta senza «results»")?;
    let punteggio = |r: &serde_json::Value| {
        r.get("relevance_score").or_else(|| r.get("score")).and_then(|s| s.as_f64()).unwrap_or(f64::NAN)
    };
    let atteso = results
        .iter()
        .find(|r| r.get("index").and_then(|i| i.as_u64()) == Some(1))
        .ok_or("nessun risultato per il documento pertinente")?;
    Ok(punteggio(atteso))
}

/// Legge i profili di servizio di una cartella `profiles/`. I file che non sono di servizio non li
/// guarda nemmeno: li legge [`crate::profile`].
pub fn list_profiles(dir: &Path) -> Vec<(ServiceProfile, Vec<crate::profile::Issue>)> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for e in entries.flatten() {
        let path = e.path();
        if path.extension().and_then(|x| x.to_str()) != Some("toml") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        if !service::is_service(&text) {
            continue;
        }
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
        if let (Some(p), issues) = service::load(&text, stem) {
            out.push((p, issues));
        }
    }
    out.sort_by(|a, b| a.0.name.cmp(&b.0.name));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_servizio_che_non_esiste_non_si_ferma_in_silenzio() {
        let s = Services::default();
        let e = s.stop("mai-acceso").unwrap_err();
        assert!(e.contains("non è acceso"), "{e}");
    }

    #[test]
    fn avviare_senza_binario_dice_quale_file_manca() {
        let (p, _) = service::load(PROFILO, "prova-embed");
        let s = Services::default();
        let e = s
            .start(StartService {
                profile: p.unwrap(),
                binary: PathBuf::from(r"X:\non\esiste\llama-server.exe"),
                model: PathBuf::from(r"X:\non\esiste\m.gguf"),
            })
            .unwrap_err();
        assert!(e.contains("binario non trovato"), "{e}");
    }

    #[test]
    fn i_profili_di_servizio_si_separano_da_quelli_principali() {
        let dir = std::env::temp_dir().join(format!("aethera-servizi-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("prova-embed.toml"), PROFILO).unwrap();
        std::fs::write(dir.join("un-principale.toml"), "schema_version = 1\nname = \"un-principale\"\n").unwrap();
        std::fs::write(dir.join("appunti.txt"), "non un profilo").unwrap();
        let trovati = list_profiles(&dir);
        assert_eq!(trovati.len(), 1, "solo il servizio: {trovati:?}");
        assert_eq!(trovati[0].0.name, "prova-embed");
        std::fs::remove_dir_all(&dir).ok();
    }

    const PROFILO: &str = r#"
schema_version = 1
name = "prova-embed"
role = "service"

[model]
file = "Qwen3-Embedding-0.6B-Q8_0.gguf"

[service]
kind = "embedding"
embed_dim = 1024

[runtime]
kind = "llama.cpp"
backend = "vulkan"
build = "b10809"

[server]
host = "127.0.0.1"
port = 18081
ctx = 8192
"#;
}
