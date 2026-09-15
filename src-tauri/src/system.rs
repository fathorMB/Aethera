//! Letture di sistema dietro un'interfaccia: su Windows dal registro e dalle API di memoria,
//! altrove i valori restano sconosciuti (`None`), mai stimati.

use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct GpuReport {
    pub name: String,
    /// Memoria dedicata dichiarata dal driver: su APU AMD riflette UMA + Variable Graphics Memory.
    pub dedicated_gib: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SystemReport {
    pub hostname: Option<String>,
    pub os: Option<String>,
    pub cpu: Option<String>,
    pub ram_total_gib: Option<f64>,
    pub ram_available_gib: Option<f64>,
    pub gpus: Vec<GpuReport>,
}

pub trait SystemProbe {
    fn report(&self) -> SystemReport;
}

pub fn probe() -> Box<dyn SystemProbe> {
    #[cfg(windows)]
    {
        Box::new(windows_probe::WindowsProbe)
    }
    #[cfg(not(windows))]
    {
        Box::new(UnknownProbe)
    }
}

pub struct UnknownProbe;

impl SystemProbe for UnknownProbe {
    fn report(&self) -> SystemReport {
        SystemReport { hostname: std::env::var("HOSTNAME").ok(), ..Default::default() }
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
fn gib(bytes: u64) -> f64 {
    (bytes as f64 / (1u64 << 30) as f64 * 100.0).round() / 100.0
}

#[cfg(windows)]
mod windows_probe {
    use super::*;
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    pub struct WindowsProbe;

    impl SystemProbe for WindowsProbe {
        fn report(&self) -> SystemReport {
            let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
            let cpu = hklm
                .open_subkey(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0")
                .and_then(|k| k.get_value::<String, _>("ProcessorNameString"))
                .ok()
                .map(|s| s.trim().to_string());
            let (ram_total_gib, ram_available_gib) = memory();
            SystemReport {
                hostname: std::env::var("COMPUTERNAME").ok(),
                os: os_name(&hklm),
                cpu,
                ram_total_gib,
                ram_available_gib,
                gpus: gpus(&hklm),
            }
        }
    }

    fn memory() -> (Option<f64>, Option<f64>) {
        let mut m = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
        match unsafe { GlobalMemoryStatusEx(&mut m) } {
            Ok(()) => (Some(gib(m.ullTotalPhys)), Some(gib(m.ullAvailPhys))),
            Err(_) => (None, None),
        }
    }

    fn os_name(hklm: &RegKey) -> Option<String> {
        let k = hklm.open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion").ok()?;
        let build: String = k.get_value("CurrentBuild").ok()?;
        // ProductName dice ancora «Windows 10» su Windows 11: conta la build.
        let family = if build.parse::<u32>().ok()? >= 22000 { "Windows 11" } else { "Windows 10" };
        Some(match k.get_value::<String, _>("DisplayVersion") {
            Ok(dv) => format!("{family} {dv} · build {build}"),
            Err(_) => format!("{family} · build {build}"),
        })
    }

    fn read_u64(k: &RegKey, name: &str) -> Option<u64> {
        if let Ok(v) = k.get_value::<u64, _>(name) {
            return Some(v);
        }
        let raw = k.get_raw_value(name).ok()?;
        let bytes: [u8; 8] = raw.bytes.get(..8)?.try_into().ok()?;
        Some(u64::from_le_bytes(bytes))
    }

    fn gpus(hklm: &RegKey) -> Vec<GpuReport> {
        let mut out = Vec::new();
        let Ok(class) =
            hklm.open_subkey(r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}")
        else {
            return out;
        };
        for name in class.enum_keys().flatten() {
            if name.len() != 4 || !name.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let Ok(k) = class.open_subkey(&name) else { continue };
            let Ok(desc) = k.get_value::<String, _>("DriverDesc") else { continue };
            // Adattatori senza memoria dichiarata (display virtuali) non dicono niente del motore.
            let Some(dedicated) = read_u64(&k, "HardwareInformation.qwMemorySize") else { continue };
            out.push(GpuReport { name: desc, dedicated_gib: Some(gib(dedicated)) });
        }
        out
    }
}
