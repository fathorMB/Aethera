<#
.SYNOPSIS
    M-16 — registro leggero del carico della macchina durante una batteria.

.DESCRIPTION
    Ogni -Secondi scrive una riga TSV: ora, CPU totale %, uso del motore 3D della GPU %, RAM
    disponibile GiB, lettura e scrittura del disco MB/s e i tre processi che hanno usato più CPU
    nell'intervallo (esclusi Idle e System Idle). Serve a riconoscere un disturbo esterno
    (antivirus, indicizzazione, aggiornamenti) quando un giro va più piano del previsto.
    Si ferma quando compare -StopFile.

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File carico.ps1 -Log <radice>\m16\carico.tsv -StopFile <radice>\stop-m16-carico
#>
param(
    [Parameter(Mandatory = $true)][string]$Log,
    [Parameter(Mandatory = $true)][string]$StopFile,
    [int]$Secondi = 60
)
$ErrorActionPreference = 'Continue'
if (Test-Path $StopFile) { Remove-Item $StopFile -Force }
if (-not (Test-Path $Log)) {
    "ora`tcpu`tgpu3d`tram_gib`tdisco_lettura_mb_s`tdisco_scrittura_mb_s`tprocessi" | Out-File -Encoding utf8 $Log
}
$contatori = @(
    '\Processor(_Total)\% Processor Time',
    '\GPU Engine(*engtype_3D)\Utilization Percentage',
    '\Memory\Available MBytes',
    '\PhysicalDisk(_Total)\Disk Read Bytes/sec',
    '\PhysicalDisk(_Total)\Disk Write Bytes/sec'
)
$core = [Environment]::ProcessorCount
$prima = @{}
Get-Process | ForEach-Object { $prima[$_.Id] = $_.CPU }
while (-not (Test-Path $StopFile)) {
    $c = Get-Counter -Counter $contatori -SampleInterval 2 -MaxSamples 1 -ErrorAction SilentlyContinue
    $v = @{}
    if ($c) {
        foreach ($s in $c.CounterSamples) {
            $k = ($s.Path -split '\\')[-1]
            if (-not $v.ContainsKey($k)) { $v[$k] = 0 }
            $v[$k] += $s.CookedValue
        }
    }
    Start-Sleep -Seconds ([Math]::Max(1, $Secondi - 2))
    $ora = @{}
    $delta = @()
    foreach ($p in Get-Process) {
        $ora[$p.Id] = $p.CPU
        if ($null -ne $p.CPU -and $prima.ContainsKey($p.Id) -and $null -ne $prima[$p.Id] -and $p.ProcessName -ne 'Idle') {
            $delta += [pscustomobject]@{ Nome = $p.ProcessName; Cpu = ($p.CPU - $prima[$p.Id]) }
        }
    }
    $prima = $ora
    $top = ($delta | Sort-Object Cpu -Descending | Select-Object -First 3 |
        ForEach-Object { '{0}={1:N0}%' -f $_.Nome, (100 * $_.Cpu / $Secondi / $core) }) -join ' '
    ("{0}`t{1:N0}`t{2:N0}`t{3:N1}`t{4:N1}`t{5:N1}`t{6}" -f (Get-Date -Format 'HH:mm:ss'),
        $v['% processor time'], $v['utilization percentage'], ($v['available mbytes'] / 1024),
        ($v['disk read bytes/sec'] / 1MB), ($v['disk write bytes/sec'] / 1MB), $top) |
        Out-File -Append -Encoding utf8 $Log
}
