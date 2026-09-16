# M-10: ferma llama-server se la macchina resta senza memoria.
# VGM 48 (T-04): soglia sulla RAM libera. VGM 64 (T-07, scelta dell'operatore del 16-09): la soglia
# e' sulla RAM *disponibile* (libera + standby), perche' le pagine mappate dal file Windows le puo'
# rilasciare; in piu' si ferma se il file di paging cresce oltre -MaxPagefileMB dall'inizio.
# Uso: powershell -File sorveglia-ram.ps1 -StopFile f -Seconds 3600 -MinGiB 2 -Log C:\AetheraData\m10\sorveglia.log
param([string]$StopFile = "", [int]$Seconds = 3600, [double]$MinGiB = 2.0, [int]$MaxPagefileMB = 4096, [int]$MaxLoadSeconds = 0,
      [string]$Log = "C:\AetheraData\m10\sorveglia.log")
$deadline = (Get-Date).AddSeconds($Seconds)
$pf0 = (Get-CimInstance Win32_PageFileUsage | Measure-Object CurrentUsage -Sum).Sum
"$((Get-Date).ToString('s')) inizio: file di paging $pf0 MB, soglia disponibile $MinGiB GiB, crescita massima $MaxPagefileMB MB" | Out-File $Log -Append -Encoding utf8
$low = 0
$ready = @{}   # pid gia' visti pronti: il controllo del caricamento non vale piu' per loro
while ((Get-Date) -lt $deadline -and -not ($StopFile -and (Test-Path $StopFile))) {
  $p = Get-Process llama-server -ErrorAction SilentlyContinue
  $avail = (Get-CimInstance Win32_PerfFormattedData_PerfOS_Memory).AvailableMBytes / 1024
  $pf = (Get-CimInstance Win32_PageFileUsage | Measure-Object CurrentUsage -Sum).Sum
  $why = $null
  if ($p -and $MinGiB -gt 0 -and $avail -lt $MinGiB) { $low++ } else { $low = 0 }
  if ($low -ge 2) { $why = "RAM disponibile $([math]::Round($avail,2)) GiB sotto $MinGiB" }
  # -MaxLoadSeconds: il motore acceso da piu' di N secondi che non risponde ancora a /health.
  $p1 = $p | Select-Object -First 1
  if ($p -and $MaxLoadSeconds -gt 0 -and -not $ready.ContainsKey($p1.Id) -and ((Get-Date) - ($p | Select-Object -First 1).StartTime).TotalSeconds -gt $MaxLoadSeconds) {
    $ok = $false
    try { $ok = (Invoke-WebRequest -UseBasicParsing -TimeoutSec 2 http://127.0.0.1:8080/health).StatusCode -eq 200 } catch {}
    if ($ok) { $ready[$p1.Id] = $true } else { $why = "non pronto dopo $MaxLoadSeconds s" }
  }
  if ($p -and ($pf - $pf0) -gt $MaxPagefileMB) { $why = "file di paging cresciuto di $($pf - $pf0) MB" }
  if ($why) {
    $p | Stop-Process -Force
    "$((Get-Date).ToString('s')) FERMATO llama-server: $why" | Out-File $Log -Append -Encoding utf8
    $low = 0
  }
  Start-Sleep -Seconds 2
}
"$((Get-Date).ToString('s')) sorveglianza finita" | Out-File $Log -Append -Encoding utf8
