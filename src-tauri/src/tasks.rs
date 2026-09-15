//! Lavori lunghi del catalogo: calcolo di uno SHA-256, download di pesi, installazione di una build.
//!
//! Durano minuti (22 GB di hash, 31 MB o 400 MB di zip), quindi non si fanno nel comando che li
//! chiede: partono in un thread e lasciano qui il loro avanzamento, che la finestra legge.
//! Ognuno si può fermare, e sullo stesso bersaglio non ne parte un secondo.
//!
//! Da non confondere con `job.rs`, che è il job object di Windows attorno al processo del motore.

use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Ricalcolo dello SHA-256 di un file già presente.
    Verify,
    Download,
    Install,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Running,
    Done,
    Failed,
    /// Fermato da chi l'ha chiesto: per un download significa che il `.part` resta.
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskView {
    pub id: String,
    pub kind: Kind,
    /// Che cosa sta lavorando: l'id della voce di catalogo o della build.
    pub target: String,
    pub state: State,
    pub done: u64,
    pub total: Option<u64>,
    /// Esito o motivo del fallimento, in parole.
    pub message: Option<String>,
    pub started: String,
    pub ended: Option<String>,
}

struct Task {
    view: TaskView,
    cancel: Arc<AtomicBool>,
}

#[derive(Clone, Default)]
pub struct Tasks {
    inner: Arc<Mutex<BTreeMap<String, Task>>>,
}

fn now() -> String {
    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
}

fn next_id() -> String {
    static N: AtomicU64 = AtomicU64::new(1);
    format!("t-{}-{}", chrono::Local::now().format("%H%M%S"), N.fetch_add(1, Ordering::Relaxed))
}

impl Tasks {
    fn lock(&self) -> MutexGuard<'_, BTreeMap<String, Task>> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Apre un lavoro. Fallisce se sullo stesso bersaglio ce n'è già uno in corso: due hash
    /// sullo stesso file da 22 GB si darebbero solo fastidio.
    pub fn start(&self, kind: Kind, target: &str) -> Result<(String, Arc<AtomicBool>), String> {
        let mut g = self.lock();
        if let Some(t) = g.values().find(|t| t.view.target == target && t.view.state == State::Running) {
            return Err(format!("c'è già un lavoro in corso su «{target}» ({})", t.view.id));
        }
        let id = next_id();
        let cancel = Arc::new(AtomicBool::new(false));
        g.insert(
            id.clone(),
            Task {
                view: TaskView {
                    id: id.clone(),
                    kind,
                    target: target.to_string(),
                    state: State::Running,
                    done: 0,
                    total: None,
                    message: None,
                    started: now(),
                    ended: None,
                },
                cancel: cancel.clone(),
            },
        );
        Ok((id, cancel))
    }

    pub fn progress(&self, id: &str, done: u64, total: Option<u64>) {
        if let Some(t) = self.lock().get_mut(id) {
            t.view.done = done;
            if total.is_some() {
                t.view.total = total;
            }
        }
    }

    /// Chiude il lavoro. `Ok` porta una frase di esito, `Err` il motivo del fallimento.
    pub fn finish(&self, id: &str, result: Result<String, String>) {
        if let Some(t) = self.lock().get_mut(id) {
            let cancelled = t.cancel.load(Ordering::Relaxed);
            t.view.state = match (&result, cancelled) {
                (Ok(_), true) => State::Cancelled,
                (Ok(_), false) => State::Done,
                (Err(_), _) => State::Failed,
            };
            t.view.message = Some(match result {
                Ok(m) | Err(m) => m,
            });
            t.view.ended = Some(now());
        }
    }

    /// Alza la bandiera di annullamento: il lavoro se ne accorge al prossimo blocco.
    pub fn cancel(&self, id: &str) -> Result<(), String> {
        let g = self.lock();
        let t = g.get(id).ok_or_else(|| format!("nessun lavoro «{id}»"))?;
        if t.view.state != State::Running {
            return Err(format!("il lavoro «{id}» è già finito"));
        }
        t.cancel.store(true, Ordering::Relaxed);
        Ok(())
    }

    pub fn running_on(&self, target: &str) -> bool {
        self.lock().values().any(|t| t.view.target == target && t.view.state == State::Running)
    }

    /// I lavori in corso e quelli finiti da poco, dal più recente.
    pub fn list(&self) -> Vec<TaskView> {
        let mut v: Vec<TaskView> = self.lock().values().map(|t| t.view.clone()).collect();
        v.sort_by(|a, b| b.id.cmp(&a.id));
        v
    }

    /// Toglie dall'elenco i lavori conclusi, così la finestra non li mostra per sempre.
    pub fn clear_finished(&self) -> usize {
        let mut g = self.lock();
        let before = g.len();
        g.retain(|_, t| t.view.state == State::Running);
        before - g.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_task_reports_progress_and_ends_done() {
        let tasks = Tasks::default();
        let (id, _) = tasks.start(Kind::Verify, "qwen3.6").unwrap();
        tasks.progress(&id, 1024, Some(4096));
        assert_eq!(tasks.list()[0].done, 1024);
        assert_eq!(tasks.list()[0].total, Some(4096));
        tasks.finish(&id, Ok("sha256 verificato".into()));
        let v = &tasks.list()[0];
        assert_eq!(v.state, State::Done);
        assert_eq!(v.message.as_deref(), Some("sha256 verificato"));
        assert!(v.ended.is_some());
    }

    #[test]
    fn two_tasks_on_the_same_target_are_refused() {
        let tasks = Tasks::default();
        let (id, _) = tasks.start(Kind::Verify, "qwen3.6").unwrap();
        let e = tasks.start(Kind::Verify, "qwen3.6").unwrap_err();
        assert!(e.contains("già un lavoro in corso"), "{e}");
        // Su un altro bersaglio invece si può.
        assert!(tasks.start(Kind::Download, "gpt-oss-20b").is_ok());
        // E quando il primo finisce, il bersaglio si libera.
        tasks.finish(&id, Ok("fatto".into()));
        assert!(!tasks.running_on("qwen3.6"));
        assert!(tasks.start(Kind::Verify, "qwen3.6").is_ok());
    }

    #[test]
    fn cancelling_marks_the_task_cancelled_not_failed() {
        let tasks = Tasks::default();
        let (id, flag) = tasks.start(Kind::Download, "qwen3-coder-next").unwrap();
        tasks.cancel(&id).unwrap();
        assert!(flag.load(Ordering::Relaxed), "il lavoro vede la bandiera");
        // Chi è stato fermato torna Ok: interrompere non è un errore.
        tasks.finish(&id, Ok("download in pausa a 1 GB".into()));
        assert_eq!(tasks.list()[0].state, State::Cancelled);
        assert!(tasks.cancel(&id).unwrap_err().contains("già finito"));
        assert!(tasks.cancel("t-inesistente").is_err());
    }

    #[test]
    fn a_failure_keeps_its_reason() {
        let tasks = Tasks::default();
        let (id, _) = tasks.start(Kind::Install, "b10989-win-vulkan-x64").unwrap();
        tasks.finish(&id, Err("SHA-256 diverso da quello dichiarato".into()));
        assert_eq!(tasks.list()[0].state, State::Failed);
        assert!(tasks.list()[0].message.as_deref().unwrap().contains("SHA-256"));
        // I finiti si possono togliere, i correnti no.
        let (running, _) = tasks.start(Kind::Verify, "altro").unwrap();
        assert_eq!(tasks.clear_finished(), 1);
        assert_eq!(tasks.list().len(), 1);
        assert_eq!(tasks.list()[0].id, running);
    }
}
