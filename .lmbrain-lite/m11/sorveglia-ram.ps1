# M-10 T-04: ferma llama-server se la RAM libera resta sotto la soglia per due campioni di fila.
# Scelta dell'operatore del 16-09 per la prova con --load-mode mmap (deroga al vincolo M-08 T-09).
# Uso: powershell -File sorveglia-ram.ps1 -Seconds 3600 -MinGiB 2 -Log C:\AetheraData\m10\sorveglia.log
param([string]$StopFile = "", [int]$Seconds = 3600, [double]$MinGiB = 2.0, [string]$Log = "C:\AetheraData\m10\sorveglia.log")
$deadline = (Get-Date).AddSeconds($Seconds)
$low = 0
while ((Get-Date) -lt $deadline -and -not ($StopFile -and (Test-Path $StopFile))) {
  $p = Get-Process llama-server -ErrorAction SilentlyContinue
  $free = (Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory / 1MB
  if ($p -and $free -lt $MinGiB) { $low++ } else { $low = 0 }
  if ($low -ge 2) {
    $p | Stop-Process -Force
    "$((Get-Date).ToString('s')) FERMATO llama-server: RAM libera $([math]::Round($free,2)) GiB sotto $MinGiB" | Out-File $Log -Append -Encoding utf8
    $low = 0
  }
  Start-Sleep -Seconds 2
}
"$((Get-Date).ToString('s')) sorveglianza finita" | Out-File $Log -Append -Encoding utf8
