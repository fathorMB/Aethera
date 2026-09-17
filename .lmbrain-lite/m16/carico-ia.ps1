# M-16: memoria delle funzioni AI di Windows e RAM disponibile, una riga ogni 3 s.
param([int]$Seconds = 900, [string]$Out = "C:\AetheraData\m16\carico-ia.csv")
if (Test-Path $Out) { Remove-Item $Out }
$fine = (Get-Date).AddSeconds($Seconds)
while ((Get-Date) -lt $fine) {
  $w = Get-CimInstance Win32_Process -Filter "Name='WorkloadsSessionHost.exe'"
  $m = Get-CimInstance Win32_PerfFormattedData_PerfOS_Memory
  [pscustomobject]@{
    at            = (Get-Date).ToString('s')
    processi_ia   = @($w).Count
    ia_gib        = [math]::Round((($w | Measure-Object WorkingSetSize -Sum).Sum)/1GB, 2)
    motore_gib    = [math]::Round(((Get-Process llama-server -ErrorAction SilentlyContinue | Measure-Object WorkingSet64 -Sum).Sum)/1GB, 2)
    disponibile   = [math]::Round($m.AvailableMBytes/1024, 2)
    standby_gib   = [math]::Round(($m.StandbyCacheNormalPriorityBytes + $m.StandbyCacheReserveBytes + $m.StandbyCacheCoreBytes)/1GB, 2)
  } | Export-Csv -Path $Out -NoTypeInformation -Encoding utf8 -Append
  Start-Sleep -Seconds 3
}
