//! Condizioni della macchina che cambiano i numeri di un avvio senza stare nel profilo: driver di
//! GPU e NPU, versione di Adrenalin, overlay di alimentazione, VGM, disco dei pesi.
//!
//! M-08 ha misurato che un aggiornamento del driver GPU sposta il prefill del 17% da solo: due
//! avvii con driver diversi non si confrontano come se differissero solo per il profilo. Ogni campo
//! che Windows non dà resta sconosciuto (`None`), mai vuoto o zero.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Un dispositivo con il suo driver, letto dalla classe di dispositivi nel registro.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Driver {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// Solo per le GPU: memoria dedicata dichiarata dal driver, che su APU AMD è UMA + VGM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dedicated_gib: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Conditions {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gpus: Vec<Driver>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub npus: Vec<Driver>,
    /// Versione commerciale di AMD Software (per esempio `26.8.1`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adrenalin: Option<String>,
    /// Schema di alimentazione attivo (GUID): su Windows 11 dice quasi sempre «Bilanciato».
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power_scheme: Option<String>,
    /// Overlay attivo a rete elettrica (GUID): è lui la «modalità di risparmio energia» delle Impostazioni.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power_overlay: Option<String>,
    /// Volume che contiene i pesi, per esempio `C:`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weights_volume: Option<String>,
    /// Modello del disco fisico sotto quel volume.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weights_disk: Option<String>,
    /// Bus del disco: NVMe, SATA, USB…
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weights_bus: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weights_free_gb: Option<f64>,
}

/// Nome in italiano degli overlay di alimentazione noti; gli altri si mostrano per GUID.
pub fn overlay_name(guid: &str) -> Option<&'static str> {
    match guid.to_ascii_lowercase().as_str() {
        "ded574b5-45a0-4f42-8737-46345c09c238" => Some("Massime prestazioni"),
        "961cc777-2547-4f9d-8174-7d86181b8a7a" => Some("Massima efficienza energetica"),
        "00000000-0000-0000-0000-000000000000" => Some("Bilanciato"),
        _ => None,
    }
}

impl Conditions {
    pub fn overlay_label(&self) -> Option<String> {
        self.power_overlay.as_deref().map(|g| overlay_name(g).map(str::to_string).unwrap_or_else(|| g.to_string()))
    }

    /// Le condizioni che rendono due avvii non confrontabili, per nome. Lo spazio libero sul disco e
    /// il disco stesso non ci stanno: cambiano il caricamento, non prefill e decode (M-08 T-03).
    pub fn comparable(&self) -> BTreeMap<&'static str, String> {
        let mut out = BTreeMap::new();
        let join = |ds: &[Driver]| ds.iter().map(|d| d.version.clone().unwrap_or_else(|| "?".into())).collect::<Vec<_>>().join(" + ");
        if !self.gpus.is_empty() {
            out.insert("driver GPU", join(&self.gpus));
            let vgm: Vec<String> = self.gpus.iter().filter_map(|g| g.dedicated_gib).map(|g| format!("{g}")).collect();
            if !vgm.is_empty() {
                out.insert("VGM", vgm.join(" + ") + " GiB");
            }
        }
        if !self.npus.is_empty() {
            out.insert("driver NPU", join(&self.npus));
        }
        if let Some(o) = self.overlay_label() {
            out.insert("alimentazione", o);
        }
        out
    }

    /// Che cosa è cambiato rispetto a un avvio precedente, in parole: `driver GPU a → b`.
    /// Una condizione sconosciuta in uno dei due non conta come cambiata: non si sa.
    pub fn changes_since(&self, before: &Conditions) -> Vec<String> {
        let (now, then) = (self.comparable(), before.comparable());
        now.iter()
            .filter_map(|(k, v)| match then.get(k) {
                Some(old) if old != v => Some(format!("{k} {old} → {v}")),
                _ => None,
            })
            .collect()
    }

    /// Riga corta per la tabella dei benchmark.
    pub fn short(&self) -> String {
        let tail = |v: &Option<String>| {
            v.as_deref()
                .map(|s| match s.rsplit_once('.') {
                    Some((_, last)) => format!("…{last}"),
                    None => s.to_string(),
                })
                .unwrap_or_else(|| "?".into())
        };
        let mut parts = Vec::new();
        if let Some(g) = self.gpus.first() {
            parts.push(format!("GPU {}", tail(&g.version)));
        }
        if let Some(n) = self.npus.first() {
            parts.push(format!("NPU {}", tail(&n.version)));
        }
        if let Some(o) = self.overlay_label() {
            parts.push(match o.as_str() {
                "Massime prestazioni" => "max".to_string(),
                "Massima efficienza energetica" => "eco".to_string(),
                "Bilanciato" => "bil".to_string(),
                other => other.chars().take(8).collect(),
            });
        }
        if let Some(v) = self.gpus.first().and_then(|g| g.dedicated_gib) {
            parts.push(format!("VGM {}", v.round()));
        }
        parts.join(" · ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(gpu: &str, npu: &str, overlay: &str) -> Conditions {
        Conditions {
            gpus: vec![Driver {
                name: "AMD Radeon(TM) 890M Graphics".into(),
                version: Some(gpu.into()),
                date: None,
                dedicated_gib: Some(48.0),
            }],
            npus: vec![Driver { name: "NPU Compute Accelerator Device".into(), version: Some(npu.into()), ..Default::default() }],
            power_overlay: Some(overlay.into()),
            weights_free_gb: Some(1769.0),
            ..Default::default()
        }
    }

    #[test]
    fn a_new_gpu_driver_is_a_change_and_free_space_is_not() {
        let before = with("32.0.22042.1", "32.0.203.314", "ded574b5-45a0-4f42-8737-46345c09c238");
        let mut now = with("32.0.31041.1004", "32.0.203.314", "ded574b5-45a0-4f42-8737-46345c09c238");
        now.weights_free_gb = Some(1200.0);
        assert_eq!(now.changes_since(&before), vec!["driver GPU 32.0.22042.1 → 32.0.31041.1004".to_string()]);
        assert!(before.changes_since(&before).is_empty());
    }

    #[test]
    fn unknown_is_not_a_change() {
        let now = with("32.0.31041.1004", "32.0.203.314", "ded574b5-45a0-4f42-8737-46345c09c238");
        assert!(now.changes_since(&Conditions::default()).is_empty());
    }

    #[test]
    fn short_line_and_overlay_names() {
        let c = with("32.0.31041.1004", "32.0.20102.3930", "DED574B5-45A0-4F42-8737-46345C09C238");
        assert_eq!(c.short(), "GPU …1004 · NPU …3930 · max · VGM 48");
        assert_eq!(overlay_name("12345678-0000-0000-0000-000000000000"), None);
        let old: Conditions = toml::from_str("").unwrap();
        assert_eq!(old, Conditions::default(), "un manifest della v1 si legge senza condizioni");
    }
}
