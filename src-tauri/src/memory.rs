//! Memoria prima e dopo il caricamento: dispositivi da `llama-server --list-devices`, contatori del
//! processo dall'interfaccia di sistema. Un valore non letto resta `None`, mai stimato.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::{Command, Stdio};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Working set oltre questa quota dei pesi: il processo tiene una seconda copia in RAM.
pub const DOUBLE_COPY_RATIO: f64 = 0.5;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub total_mib: u64,
    pub free_mib: u64,
}

/// `  Vulkan0: AMD Radeon(TM) 890M Graphics (73548 MiB, 69870 MiB free)`
pub fn parse_devices(text: &str) -> Vec<Device> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            let (id, rest) = line.split_once(": ")?;
            if id.contains(' ') || !id.chars().last()?.is_ascii_digit() {
                return None;
            }
            let open = rest.rfind('(')?;
            let inner = rest[open + 1..].strip_suffix(')')?;
            let (total, free) = inner.split_once(", ")?;
            Some(Device {
                id: id.to_string(),
                name: rest[..open].trim().to_string(),
                total_mib: total.strip_suffix(" MiB")?.trim().parse().ok()?,
                free_mib: free.strip_suffix(" MiB free")?.trim().parse().ok()?,
            })
        })
        .collect()
}

pub fn list_devices(binary: &Path) -> Result<Vec<Device>, String> {
    let mut cmd = Command::new(binary);
    cmd.arg("--list-devices").stdin(Stdio::null());
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    let out = cmd.output().map_err(|e| format!("{} --list-devices: {e}", binary.display()))?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    Ok(parse_devices(&text))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MemoryBefore {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vram_total_mib: Option<u64>,
    /// Memoria libera dell'heap DEVICE_LOCAL secondo il backend, subito prima dell'avvio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vram_free_mib: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ram_available_gib: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MemoryAfter {
    pub at: String,
    /// Millisecondi dall'avvio del processo alla lettura.
    pub after_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vram_dedicated_gib: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vram_shared_gib: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_set_gib: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ram_available_gib: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub double_copy: Option<bool>,
    pub ram_margin_gib: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_ok: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MemorySection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<MemoryBefore>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_load: Option<MemoryAfter>,
}

pub fn before(devices: &[Device], ram_available_gib: Option<f64>) -> MemoryBefore {
    // Con più dispositivi conta quello con più memoria: è quello su cui va il modello con -ngl 999.
    let d = devices.iter().max_by_key(|d| d.total_mib);
    MemoryBefore {
        device: d.map(|d| format!("{}: {}", d.id, d.name)),
        vram_total_mib: d.map(|d| d.total_mib),
        vram_free_mib: d.map(|d| d.free_mib),
        ram_available_gib,
    }
}

pub fn double_copy(working_set_gib: Option<f64>, model_size_bytes: u64) -> Option<bool> {
    let ws = working_set_gib?;
    if model_size_bytes == 0 {
        return None;
    }
    let weights_gib = model_size_bytes as f64 / (1u64 << 30) as f64;
    Some(ws >= weights_gib * DOUBLE_COPY_RATIO)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_list_devices() {
        let text = "ggml_vulkan: Found 1 Vulkan devices:\nAvailable devices:\n  Vulkan0: AMD Radeon(TM) 890M Graphics (73548 MiB, 69870 MiB free)\n";
        let d = parse_devices(text);
        assert_eq!(
            d,
            vec![Device { id: "Vulkan0".into(), name: "AMD Radeon(TM) 890M Graphics".into(), total_mib: 73548, free_mib: 69870 }]
        );
        assert_eq!(before(&d, Some(45.1)).vram_free_mib, Some(69870));
        assert!(parse_devices("Available devices:\n").is_empty());
    }

    #[test]
    fn double_copy_from_working_set() {
        let g1 = 22_285_080_192;
        assert_eq!(double_copy(Some(2.0), g1), Some(false));
        assert_eq!(double_copy(Some(21.5), g1), Some(true));
        assert_eq!(double_copy(None, g1), None);
    }
}
