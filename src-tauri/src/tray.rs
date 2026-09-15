//! Testi e voci della tray derivati dallo stato del motore.

use crate::engine::EngineStatus;

#[derive(Debug, Clone, PartialEq)]
pub struct TrayView {
    pub state: String,
    pub model: String,
    pub info: String,
    pub tooltip: String,
    pub protected: bool,
    pub can_stop: bool,
    pub can_restart: bool,
    pub has_run: bool,
}

pub fn duration(seconds: u64) -> String {
    let (d, h, m) = (seconds / 86400, (seconds % 86400) / 3600, (seconds % 3600) / 60);
    if d > 0 {
        format!("{d} g {h} h")
    } else if h > 0 {
        format!("{h} h {m} m")
    } else if m > 0 {
        format!("{m} m {} s", seconds % 60)
    } else {
        format!("{seconds} s")
    }
}

fn it(x: f64, decimals: usize) -> String {
    format!("{x:.decimals$}").replace('.', ",")
}

pub fn view(s: &EngineStatus, protected: bool) -> TrayView {
    let mut v = TrayView {
        state: "SPENTO".into(),
        model: "nessun motore acceso".into(),
        info: String::new(),
        tooltip: "Aethera · motore spento".into(),
        protected,
        can_stop: false,
        can_restart: false,
        has_run: false,
    };
    match s {
        EngineStatus::Off { last } => {
            v.can_restart = last.is_some();
        }
        EngineStatus::Exited { finished } => {
            v.state = "USCITO CON ERRORE".into();
            v.model = finished.run.profile.clone();
            v.info = format!("codice {}", finished.code.map(|c| c.to_string()).unwrap_or_else(|| "sconosciuto".into()));
            v.tooltip = format!("Aethera · {} uscito con errore", finished.run.profile);
            v.can_restart = true;
        }
        EngineStatus::Orphan { orphan, .. } => {
            v.state = "ORFANO".into();
            v.model = format!("{} · :{}", orphan.alias.as_deref().unwrap_or("alias sconosciuto"), orphan.port);
            v.info = format!("PID {} non avviato da questo Aethera", orphan.pid);
            v.tooltip = format!("Aethera · orfano su :{}", orphan.port);
        }
        EngineStatus::Loading { run, elapsed_s, usage, .. } => {
            v.state = if usage.in_use { "IN CARICAMENTO · IN USO".into() } else { "IN CARICAMENTO".into() };
            v.model = format!("{} · {}", run.profile, run.base_url.rsplit(':').next().map(|p| format!(":{p}")).unwrap_or_default());
            v.info = format!("in caricamento da {}", duration(*elapsed_s));
            v.tooltip = format!("Aethera · {} in caricamento", run.profile);
            v.can_stop = !usage.in_use;
            v.has_run = true;
        }
        EngineStatus::Ready { run, uptime_s, usage, telemetry, degraded, divergences, .. } => {
            let base = if !degraded.is_empty() {
                "DEGRADATO"
            } else if !divergences.is_empty() {
                "DIVERGENTE"
            } else {
                "PRONTO"
            };
            v.state = if usage.in_use { format!("{base} · IN USO") } else { base.into() };
            v.model = format!("{} · {}", run.profile, run.base_url.rsplit(':').next().map(|p| format!(":{p}")).unwrap_or_default());
            let mut info = vec![format!("acceso da {}", duration(*uptime_s))];
            if let Some(d) = telemetry.decode_median {
                info.push(format!("{} tok/s", it(d, 1)));
            }
            if let Some(c) = telemetry.cache_share {
                info.push(format!("cache {} %", it(c * 100.0, 0)));
            }
            v.info = info.join(" · ");
            v.tooltip = format!("Aethera · {} · {}", run.profile, v.state.to_lowercase());
            v.can_stop = !usage.in_use;
            v.can_restart = !usage.in_use;
            v.has_run = true;
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_view() {
        let v = view(&EngineStatus::Off { last: None }, false);
        assert_eq!(v.state, "SPENTO");
        assert!(!v.can_stop && !v.can_restart && !v.has_run);
        assert_eq!(duration(4380), "1 h 13 m");
    }
}
