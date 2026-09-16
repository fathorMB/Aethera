# M-10 T-03 (da M-08 T-09) — contatori di memoria durante il caricamento e il lavoro del motore.
#
# RAMMap e' una finestra e non si puo' guidare da qui: gli stessi fatti si leggono dai contatori.
# Quello che distingue le due modalita' di caricamento e' «Private Bytes» del processo:
#   --load-mode none  -> il processo copia i pesi nella propria memoria privata: Private Bytes sale
#                        fino a circa la dimensione del modello (doppia copia con la cache del file);
#   --load-mode mmap  -> le pagine restano del file: Private Bytes resta basso, mentre il working
#                        set condiviso e la cache di sistema crescono.
#
# M-10: working_set - ws_private = pagine condivise del processo, cioe' le pagine mappate dal file
# (la tabella per_layer_token_embd letta con --lazy-mode); read_gib = byte letti dal processo
# (quanto della tabella e' venuto dal disco). La doppia copia resta in private_gib.
#
# Uso: powershell -File campiona-memoria.ps1 -Seconds 1800 -Out <radice>\m10\T-04-memoria.csv

param(
  [string]$StopFile = "",
  [int]$Seconds = 600,
  [int]$IntervalMs = 2000,
  [Parameter(Mandatory = $true)][string]$Out
)

$Out = [System.IO.Path]::GetFullPath($Out)
$dir = Split-Path $Out -Parent
if ($dir) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
$n = 0
if (Test-Path $Out) { Remove-Item $Out }
$deadline = (Get-Date).AddSeconds($Seconds)
while ((Get-Date) -lt $deadline -and -not ($StopFile -and (Test-Path $StopFile))) {
  $p = Get-Process llama-server -ErrorAction SilentlyContinue | Sort-Object WS -Descending | Select-Object -First 1
  $os = Get-CimInstance Win32_OperatingSystem
  $perf = $null
  if ($p) {
    $perf = Get-CimInstance Win32_PerfRawData_PerfProc_Process -Filter "IDProcess=$($p.Id)" -ErrorAction SilentlyContinue
  }
  $mem = Get-CimInstance Win32_PerfRawData_PerfOS_Memory -ErrorAction SilentlyContinue
  $row = [pscustomobject]@{
    at                 = (Get-Date).ToString("s")
    pid                = if ($p) { $p.Id } else { $null }
    working_set_gib    = if ($perf) { [math]::Round($perf.WorkingSet / 1GB, 3) } else { $null }
    ws_private_gib     = if ($perf) { [math]::Round($perf.WorkingSetPrivate / 1GB, 3) } else { $null }
    ws_mapped_gib      = if ($perf) { [math]::Round(($perf.WorkingSet - $perf.WorkingSetPrivate) / 1GB, 3) } else { $null }
    read_gib           = if ($perf) { [math]::Round($perf.IOReadBytesPersec / 1GB, 3) } else { $null }
    private_gib        = if ($perf) { [math]::Round($perf.PrivateBytes / 1GB, 3) } else { $null }
    virtual_gib        = if ($perf) { [math]::Round($perf.VirtualBytes / 1GB, 3) } else { $null }
    page_faults        = if ($perf) { $perf.PageFaultsPersec } else { $null }
    cache_gib          = if ($mem) { [math]::Round($mem.CacheBytes / 1GB, 3) } else { $null }
    standby_gib        = if ($mem) { [math]::Round(($mem.StandbyCacheNormalPriorityBytes + $mem.StandbyCacheReserveBytes + $mem.StandbyCacheCoreBytes) / 1GB, 3) } else { $null }
    # ram_available_gib e' la RAM *libera*; avail_gib aggiunge lo standby (M-10, VGM 64)
    avail_gib          = if ($mem) { [math]::Round($mem.AvailableBytes / 1GB, 3) } else { $null }
    pagefile_mb        = (Get-CimInstance Win32_PageFileUsage | Measure-Object CurrentUsage -Sum).Sum
    ram_available_gib  = [math]::Round($os.FreePhysicalMemory / 1MB, 3)
  }
  # Una riga per campione, subito: se il campionatore si ferma, quello che ha visto resta.
  $row | Export-Csv -Path $Out -NoTypeInformation -Encoding utf8 -Append
  $n++
  Start-Sleep -Milliseconds $IntervalMs
}
Write-Output "$n campioni in $Out"
