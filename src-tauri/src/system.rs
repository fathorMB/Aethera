//! Letture di sistema dietro un'interfaccia: su Windows dal registro e dalle API di memoria,
//! altrove i valori restano sconosciuti (`None`), mai stimati.

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
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::NetworkManagement::IpHelper::{GetExtendedTcpTable, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER};
    use windows::Win32::Networking::WinSock::AF_INET;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, GetDiskFreeSpaceExW, GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_NORMAL,
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
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
