//! Letture di sistema dietro un'interfaccia: su Windows dal registro e dalle API di memoria,
//! altrove i valori restano sconosciuti (`None`), mai stimati.

use crate::conditions::Conditions;
use serde::Serialize;
use std::path::Path;

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

/// Memoria di un processo: contatori `GPU Process Memory` e working set.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct ProcessMemory {
    pub gpu_dedicated_gib: Option<f64>,
    pub gpu_shared_gib: Option<f64>,
    pub working_set_gib: Option<f64>,
}

pub trait SystemProbe {
    fn report(&self) -> SystemReport;

    fn ram_available_gib(&self) -> Option<f64> {
        self.report().ram_available_gib
    }

    fn process_memory(&self, _pid: u32) -> ProcessMemory {
        ProcessMemory::default()
    }

    /// Processo in ascolto su una porta TCP IPv4 locale.
    fn listening_pid(&self, _port: u16) -> Option<u32> {
        None
    }

    /// Nome del file eseguibile di un processo (per esempio `llama-server.exe`).
    fn process_name(&self, _pid: u32) -> Option<String> {
        None
    }

    fn terminate(&self, _pid: u32) -> Result<(), String> {
        Err("terminare un processo non è supportato su questo sistema".into())
    }

    /// Quanti nomi puntano a questo stesso file: `lms import -L` di LM Studio crea un hard link.
    /// Un file con più collegamenti non si cancella senza dirlo.
    fn hard_links(&self, _path: &Path) -> Option<u32> {
        None
    }

    /// Spazio libero sul volume che contiene il percorso: si controlla prima di un download.
    fn free_disk_bytes(&self, _path: &Path) -> Option<u64> {
        None
    }

    /// Identità del file sul volume (serial del volume, indice del file): due nomi con la stessa
    /// identità sono lo stesso contenuto, cioè hard link. Serve a non registrare due volte gli
    /// stessi pesi quando arrivano da fuori la cartella dichiarata.
    fn file_id(&self, _path: &Path) -> Option<(u32, u64)> {
        None
    }

    /// Driver, alimentazione e disco dei pesi (`weights_dir`), letti all'avvio per il manifest.
    fn conditions(&self, _weights_dir: Option<&Path>) -> Conditions {
        Conditions::default()
    }
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
    use crate::conditions::Driver;
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::NetworkManagement::IpHelper::{GetExtendedTcpTable, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER};
    use windows::Win32::Networking::WinSock::AF_INET;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, GetDiskFreeSpaceExW, GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_NORMAL,
        FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows::Win32::System::Ioctl::{
        PropertyStandardQuery, StorageDeviceProperty, IOCTL_STORAGE_QUERY_PROPERTY, STORAGE_DEVICE_DESCRIPTOR,
        STORAGE_PROPERTY_QUERY,
    };
    use windows::Win32::System::IO::DeviceIoControl;
    use windows::Win32::System::Performance::{
        PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW, PdhOpenQueryW,
        PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_LARGE, PDH_HCOUNTER, PDH_HQUERY, PDH_MORE_DATA,
    };
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, TerminateProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        PROCESS_TERMINATE,
    };
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

        fn ram_available_gib(&self) -> Option<f64> {
            memory().1
        }

        fn process_memory(&self, pid: u32) -> ProcessMemory {
            ProcessMemory {
                gpu_dedicated_gib: gpu_process_counter(pid, "Dedicated Usage").map(gib),
                gpu_shared_gib: gpu_process_counter(pid, "Shared Usage").map(gib),
                working_set_gib: working_set(pid).map(gib),
            }
        }

        fn listening_pid(&self, port: u16) -> Option<u32> {
            listening_pid(port)
        }

        fn process_name(&self, pid: u32) -> Option<String> {
            process_name(pid)
        }

        fn terminate(&self, pid: u32) -> Result<(), String> {
            unsafe {
                let h = OpenProcess(PROCESS_TERMINATE, false, pid).map_err(|e| format!("OpenProcess {pid}: {e}"))?;
                let r = TerminateProcess(h, 1).map_err(|e| format!("TerminateProcess {pid}: {e}"));
                let _ = CloseHandle(h);
                r
            }
        }

        fn hard_links(&self, path: &Path) -> Option<u32> {
            hard_links(path)
        }

        fn free_disk_bytes(&self, path: &Path) -> Option<u64> {
            free_disk_bytes(path)
        }

        fn file_id(&self, path: &Path) -> Option<(u32, u64)> {
            file_id(path)
        }

        fn conditions(&self, weights_dir: Option<&Path>) -> Conditions {
            let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
            let power = hklm.open_subkey(r"SYSTEM\CurrentControlSet\Control\Power\User\PowerSchemes").ok();
            let power_value = |name: &str| {
                power.as_ref().and_then(|k| k.get_value::<String, _>(name).ok()).filter(|v| !v.trim().is_empty())
            };
            let volume = weights_dir.and_then(volume_letter);
            let (weights_disk, weights_bus) = volume.map(disk_of_volume).unwrap_or((None, None));
            Conditions {
                // Come in `gpus`: un adattatore senza memoria dichiarata è un display virtuale.
                gpus: drivers(&hklm, GPU_CLASS).into_iter().filter(|d| d.dedicated_gib.is_some()).collect(),
                npus: drivers(&hklm, NPU_CLASS),
                adrenalin: adrenalin(&hklm),
                power_scheme: power_value("ActivePowerScheme"),
                power_overlay: power_value("ActiveOverlayAcPowerScheme"),
                weights_volume: volume.map(|l| format!("{l}:")),
                weights_disk,
                weights_bus,
                weights_free_gb: weights_dir.and_then(free_disk_bytes).map(|b| (b as f64 / 1e7).round() / 100.0),
            }
        }
    }

    const GPU_CLASS: &str = "{4d36e968-e325-11ce-bfc1-08002be10318}";
    /// Classe «ComputeAccelerator» di Windows: le NPU (AMD XDNA, Intel AI Boost) stanno qui.
    const NPU_CLASS: &str = "{f01a9d53-3ff6-48d2-9f97-c8a7004be10c}";

    /// `8-17-2026` → `2026-08-17`.
    fn driver_date(raw: &str) -> String {
        match raw.split('-').collect::<Vec<_>>().as_slice() {
            [m, d, y] if y.len() == 4 => format!("{y}-{m:0>2}-{d:0>2}"),
            _ => raw.to_string(),
        }
    }

    fn drivers(hklm: &RegKey, class: &str) -> Vec<Driver> {
        let mut out = Vec::new();
        let Ok(k) = hklm.open_subkey(format!(r"SYSTEM\CurrentControlSet\Control\Class\{class}")) else { return out };
        for name in k.enum_keys().flatten() {
            if name.len() != 4 || !name.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let Ok(d) = k.open_subkey(&name) else { continue };
            let Ok(desc) = d.get_value::<String, _>("DriverDesc") else { continue };
            out.push(Driver {
                name: desc,
                version: d.get_value::<String, _>("DriverVersion").ok(),
                date: d.get_value::<String, _>("DriverDate").ok().map(|s| driver_date(&s)),
                dedicated_gib: read_u64(&d, "HardwareInformation.qwMemorySize").map(gib),
            });
        }
        out
    }

    /// Versione commerciale di AMD Software, dalla sua voce di disinstallazione.
    fn adrenalin(hklm: &RegKey) -> Option<String> {
        let k = hklm.open_subkey(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall").ok()?;
        k.enum_keys().flatten().find_map(|name| {
            let e = k.open_subkey(&name).ok()?;
            let display: String = e.get_value("DisplayName").ok()?;
            if display != "AMD Software" {
                return None;
            }
            e.get_value::<String, _>("DisplayVersion").ok()
        })
    }

    fn volume_letter(path: &Path) -> Option<char> {
        let s = path.to_str()?;
        let s = s.strip_prefix(r"\\?\").unwrap_or(s);
        let mut c = s.chars();
        let letter = c.next()?.to_ascii_uppercase();
        (letter.is_ascii_alphabetic() && c.next() == Some(':')).then_some(letter)
    }

    fn bus_name(bus: i32) -> String {
        match bus {
            1 => "SCSI".into(),
            3 => "ATA".into(),
            7 => "USB".into(),
            8 => "RAID".into(),
            10 => "SAS".into(),
            11 => "SATA".into(),
            12 => "SD".into(),
            13 => "MMC".into(),
            15 => "file virtuale".into(),
            16 => "Spazi di archiviazione".into(),
            17 => "NVMe".into(),
            other => format!("bus {other}"),
        }
    }

    /// Modello e bus del disco sotto un volume, con `IOCTL_STORAGE_QUERY_PROPERTY`: non serve essere
    /// amministratore, basta aprire il volume senza diritti di lettura.
    fn disk_of_volume(letter: char) -> (Option<String>, Option<String>) {
        unsafe {
            let w: Vec<u16> = format!(r"\\.\{letter}:").encode_utf16().chain(std::iter::once(0)).collect();
            let Ok(h) = CreateFileW(
                PCWSTR(w.as_ptr()),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            ) else {
                return (None, None);
            };
            let query = STORAGE_PROPERTY_QUERY {
                PropertyId: StorageDeviceProperty,
                QueryType: PropertyStandardQuery,
                AdditionalParameters: [0],
            };
            let mut buf = vec![0u8; 4096];
            let mut returned = 0u32;
            let ok = DeviceIoControl(
                h,
                IOCTL_STORAGE_QUERY_PROPERTY,
                Some(&query as *const STORAGE_PROPERTY_QUERY as *const std::ffi::c_void),
                std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
                Some(buf.as_mut_ptr().cast()),
                buf.len() as u32,
                Some(&mut returned),
                None,
            )
            .is_ok();
            let _ = CloseHandle(h);
            let len = returned as usize;
            if !ok || len < std::mem::size_of::<STORAGE_DEVICE_DESCRIPTOR>() {
                return (None, None);
            }
            let desc = std::ptr::read_unaligned(buf.as_ptr() as *const STORAGE_DEVICE_DESCRIPTOR);
            let text = |offset: u32| -> Option<String> {
                let start = offset as usize;
                if start == 0 || start >= len {
                    return None;
                }
                let end = buf[start..len].iter().position(|b| *b == 0).map_or(len, |p| start + p);
                let s = String::from_utf8_lossy(&buf[start..end]).trim().to_string();
                (!s.is_empty()).then_some(s)
            };
            let model = match (text(desc.VendorIdOffset), text(desc.ProductIdOffset)) {
                (Some(v), Some(p)) if !p.starts_with(&v) => Some(format!("{v} {p}")),
                (_, Some(p)) => Some(p),
                (v, None) => v,
            };
            (model, Some(bus_name(desc.BusType.0)))
        }
    }

    fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        s.encode_wide().chain(std::iter::once(0)).collect()
    }

    /// `nNumberOfLinks` dal handle: 1 significa che quel file esiste con un nome solo.
    fn hard_links(path: &Path) -> Option<u32> {
        unsafe {
            let w = wide(path.as_os_str());
            // Accesso 0: bastano i metadati, e non disturba un file che il motore sta usando.
            let h = CreateFileW(
                PCWSTR(w.as_ptr()),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
            .ok()?;
            let mut info = BY_HANDLE_FILE_INFORMATION::default();
            let r = GetFileInformationByHandle(h, &mut info).ok().map(|_| info.nNumberOfLinks);
            let _ = CloseHandle(h);
            r
        }
    }

    /// Volume e indice del file: identici significa che i due nomi sono lo stesso file.
    fn file_id(path: &Path) -> Option<(u32, u64)> {
        unsafe {
            let w = wide(path.as_os_str());
            let h = CreateFileW(
                PCWSTR(w.as_ptr()),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
            .ok()?;
            let mut info = BY_HANDLE_FILE_INFORMATION::default();
            let r = GetFileInformationByHandle(h, &mut info)
                .ok()
                .map(|_| (info.dwVolumeSerialNumber, ((info.nFileIndexHigh as u64) << 32) | info.nFileIndexLow as u64));
            let _ = CloseHandle(h);
            r
        }
    }

    fn free_disk_bytes(path: &Path) -> Option<u64> {
        // Il percorso può non esistere ancora (file da scaricare): si sale alla prima cartella che c'è.
        let mut dir = path.to_path_buf();
        while !dir.is_dir() {
            if !dir.pop() {
                return None;
            }
        }
        unsafe {
            let w = wide(dir.as_os_str());
            let mut free = 0u64;
            GetDiskFreeSpaceExW(PCWSTR(w.as_ptr()), Some(&mut free), None, None).ok()?;
            Some(free)
        }
    }

    /// Somma di un contatore `\GPU Process Memory(pid_<pid>_*)\<name>` su tutti gli adattatori.
    fn gpu_process_counter(pid: u32, name: &str) -> Option<u64> {
        unsafe {
            let mut query = PDH_HQUERY(std::ptr::null_mut());
            if PdhOpenQueryW(PCWSTR::null(), 0, &mut query) != 0 {
                return None;
            }
            let result = (|| {
                let path: Vec<u16> =
                    format!("\\GPU Process Memory(pid_{pid}_*)\\{name}").encode_utf16().chain(std::iter::once(0)).collect();
                let mut counter = PDH_HCOUNTER(std::ptr::null_mut());
                if PdhAddEnglishCounterW(query, PCWSTR(path.as_ptr()), 0, &mut counter) != 0 {
                    return None;
                }
                if PdhCollectQueryData(query) != 0 {
                    return None;
                }
                let (mut size, mut count) = (0u32, 0u32);
                if PdhGetFormattedCounterArrayW(counter, PDH_FMT_LARGE, &mut size, &mut count, None) != PDH_MORE_DATA {
                    return None;
                }
                let item = std::mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>();
                let mut buf: Vec<PDH_FMT_COUNTERVALUE_ITEM_W> =
                    (0..(size as usize).div_ceil(item) + 1).map(|_| Default::default()).collect();
                if PdhGetFormattedCounterArrayW(counter, PDH_FMT_LARGE, &mut size, &mut count, Some(buf.as_mut_ptr())) != 0 {
                    return None;
                }
                let valid: Vec<i64> = buf[..count as usize]
                    .iter()
                    .filter(|i| i.FmtValue.CStatus <= 1)
                    .map(|i| i.FmtValue.Anonymous.largeValue)
                    .collect();
                (!valid.is_empty()).then(|| valid.iter().map(|v| (*v).max(0) as u64).sum())
            })();
            PdhCloseQuery(query);
            result
        }
    }

    fn working_set(pid: u32) -> Option<u64> {
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut c = PROCESS_MEMORY_COUNTERS { cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32, ..Default::default() };
            let r = GetProcessMemoryInfo(h, &mut c, c.cb).ok().map(|_| c.WorkingSetSize as u64);
            let _ = CloseHandle(h);
            r
        }
    }

    fn process_name(pid: u32) -> Option<String> {
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let r = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).ok();
            let _ = CloseHandle(h);
            r?;
            let full = String::from_utf16_lossy(&buf[..len as usize]);
            full.rsplit(['\\', '/']).next().map(str::to_string)
        }
    }

    fn listening_pid(port: u16) -> Option<u32> {
        unsafe {
            let mut size = 0u32;
            let _ = GetExtendedTcpTable(None, &mut size, false, AF_INET.0 as u32, TCP_TABLE_OWNER_PID_LISTENER, 0);
            if size == 0 {
                return None;
            }
            let mut buf = vec![0u32; (size as usize).div_ceil(4) + 1];
            if GetExtendedTcpTable(Some(buf.as_mut_ptr().cast()), &mut size, false, AF_INET.0 as u32, TCP_TABLE_OWNER_PID_LISTENER, 0)
                != 0
            {
                return None;
            }
            let table = &*(buf.as_ptr() as *const MIB_TCPTABLE_OWNER_PID);
            let rows = std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize);
            // La porta sta nei 16 bit bassi, in ordine di rete.
            rows.iter().find(|r| u16::from_be((r.dwLocalPort & 0xffff) as u16) == port).map(|r| r.dwOwningPid)
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
