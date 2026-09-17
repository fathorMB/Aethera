<#
.SYNOPSIS
    M-14 — build locale di llama.cpp: un tag di ggml-org più una serie corta di rami patch.

.DESCRIPTION
    Dato un tag b<numero> e l'elenco dei rami patch (patch/<nome>, in ordine):
      1. fetch del tag e di master da upstream (ggml-org);
      2. rebase di ogni ramo patch sul tag (i commit della patch sono quelli che non stanno in
         upstream/master); se un rebase ha conflitti lo annulla, dice quali file e si ferma;
      3. ramo moro-ai = tag + i commit di ogni patch, nell'ordine dato (cherry-pick);
      4. build Release Vulkan x64 con MSVC (ambiente di VsDevCmd, CMake e Ninja di Build Tools);
      5. copia di eseguibili e DLL in <radice>\builds\llama-b<numero>+moro<n>-win-vulkan-x64;
      6. file di provenienza (provenienza.toml) accanto all'eseguibile;
      7. voce [[build]] in <radice>\machine.toml, dopo una copia di sicurezza, senza toccare le altre.

    moro0 è il tag liscio compilato qui; moro<n> con n >= 1 è una serie con almeno una patch.
    Il numero di build dichiarato all'eseguibile resta quello del tag (-DLLAMA_BUILD_NUMBER), il
    commit è quello vero di moro-ai: --version dice «build 10991, commit <moro-ai>».

    Il repo del fork non ha un remote di push: niente di questo va su GitHub.

.EXAMPLE
    .\build.ps1 -Tag b10991 -Serie 0
.EXAMPLE
    .\build.ps1 -Tag b10991 -Patch patch/int8-coopmat -Serie 1
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][ValidatePattern('^b\d+$')][string]$Tag,
    [string[]]$Patch = @(),
    [int]$Serie = -1,
    # Radice dati di Aethera: parametro o variabile d'ambiente AETHERA_RADICE.
    [string]$Radice = $env:AETHERA_RADICE,
    # Repo del fork: per difetto <radice>\src\llama.cpp.
    [string]$Repo = '',
    [switch]$SenzaFetch,
    [switch]$SenzaMachineToml,
    # Riusa la cartella di build invece di ripartire da zero (il tempo annotato non è più «pulito»).
    [switch]$Incrementale,
    # Sovrascrive una build con lo stesso id già presente in builds\.
    [switch]$Sovrascrivi
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 3

function Fallisci([string]$msg, [int]$codice = 1) {
    Write-Host "ERRORE: $msg" -ForegroundColor Red
    exit $codice
}

# EseguiGit: git nel repo del fork, un codice diverso da zero è un errore.
# ProvaGit: lo stesso, ma il codice resta in $script:UltimoCodice e decide chi chiama.
function EseguiGit {
    $Argomenti = $args
    $vecchio = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    $out = & git -C $script:Repo @Argomenti
    $codice = $LASTEXITCODE
    $ErrorActionPreference = $vecchio
    if ($codice -ne 0) { throw "git $($Argomenti -join ' ') è uscito con $codice" }
    return $out
}

function ProvaGit {
    $Argomenti = $args
    $vecchio = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    $out = & git -C $script:Repo @Argomenti
    $script:UltimoCodice = $LASTEXITCODE
    $ErrorActionPreference = $vecchio
    return $out
}

function TomlStringa([string]$s) {
    return '"' + ($s -replace '\\', '\\' -replace '"', '\"') + '"'
}

# ------------------------------------------------------------------ parametri e controlli

if ([string]::IsNullOrWhiteSpace($Radice)) { Fallisci 'manca la radice dati: passa -Radice o imposta AETHERA_RADICE' }
$Radice = (Resolve-Path $Radice).Path
if ([string]::IsNullOrWhiteSpace($Repo)) { $Repo = Join-Path $Radice 'src\llama.cpp' }
if (-not (Test-Path (Join-Path $Repo '.git'))) { Fallisci "nessun repo git in $Repo (vedi .lmbrain-lite/fork/README.md, «Preparare il repo»)" }
$script:Repo = (Resolve-Path $Repo).Path

if ($Serie -lt 0) { $Serie = $Patch.Count }
if ($Serie -eq 0 -and $Patch.Count -gt 0) { Fallisci 'moro0 è il tag liscio: con delle patch la serie deve essere >= 1' }
if ($Serie -gt 0 -and $Patch.Count -eq 0) { Fallisci "moro$Serie senza patch: una serie >= 1 dichiara almeno un ramo" }
foreach ($p in $Patch) {
    if ($p -notmatch '^patch/[A-Za-z0-9._-]+$') { Fallisci "ramo patch «$p» non valido: la forma è patch/<nome>" }
}

$Backend = 'vulkan'
$Etichetta = "$Tag+moro$Serie"
$Id = "$Etichetta-$Backend"
$Destinazione = Join-Path $Radice "builds\llama-$Etichetta-win-$Backend-x64"
$MachineToml = Join-Path $Radice 'machine.toml'

if ((Test-Path $Destinazione) -and -not $Sovrascrivi) {
    Fallisci "$Destinazione esiste già: usa -Sovrascrivi, oppure un altro numero di serie"
}

$sporco = ProvaGit status --porcelain --untracked-files=no
if ($sporco) { Fallisci "il repo del fork ha modifiche non salvate:`n$($sporco -join "`n")" }

Write-Host "== build $Id da $Tag con $($Patch.Count) patch =="
$inizio = Get-Date

# ------------------------------------------------------------------ fetch

if (-not $SenzaFetch) {
    Write-Host '-- fetch da upstream'
    EseguiGit fetch --quiet upstream "refs/tags/${Tag}:refs/tags/${Tag}" --no-tags | Out-Null
    EseguiGit fetch --quiet upstream master | Out-Null
}
$TagCommit = (EseguiGit rev-parse "$Tag^{commit}").Trim()
Write-Host "   $Tag = $TagCommit"

# ------------------------------------------------------------------ rebase dei rami patch

$rami = @()
foreach ($p in $Patch) {
    $prima = (EseguiGit rev-parse --verify "refs/heads/$p").Trim()
    Write-Host "-- rebase di $p ($($prima.Substring(0,9))) su $Tag"
    $out = ProvaGit rebase --quiet --onto $TagCommit upstream/master $p
    if ($script:UltimoCodice -ne 0) {
        $conflitti = ProvaGit diff --name-only --diff-filter=U
        ProvaGit rebase --abort | Out-Null
        ProvaGit checkout --quiet --detach $TagCommit | Out-Null
        Fallisci ("il rebase di $p su $Tag ha conflitti in:`n  " + ($conflitti -join "`n  ") +
            "`nIl rebase è stato annullato e il ramo è com'era. Se il conflitto non è meccanico, il ramo si butta (regola 1).") 2
    }
    $dopo = (EseguiGit rev-parse "refs/heads/$p").Trim()
    $commit = @(EseguiGit log --format='%H %s' "$TagCommit..$dopo")
    [array]::Reverse($commit)
    Write-Host "   $($commit.Count) commit, ora $($dopo.Substring(0,9))"
    $rami += [pscustomobject]@{ Ramo = $p; Prima = $prima; Dopo = $dopo; Commit = $commit }
}

# ------------------------------------------------------------------ ramo moro-ai

Write-Host '-- moro-ai = tag + patch'
EseguiGit checkout --quiet -B moro-ai $TagCommit | Out-Null
foreach ($r in $rami) {
    $out = ProvaGit cherry-pick --allow-empty "$TagCommit..$($r.Dopo)"
    if ($script:UltimoCodice -ne 0) {
        $conflitti = ProvaGit diff --name-only --diff-filter=U
        ProvaGit cherry-pick --abort | Out-Null
        EseguiGit checkout --quiet -B moro-ai $TagCommit | Out-Null
        Fallisci ("$($r.Ramo) non si applica sopra le patch precedenti; conflitti in:`n  " + ($conflitti -join "`n  ") +
            "`nmoro-ai è tornato al tag.") 2
    }
}
$MoroCommit = (EseguiGit rev-parse HEAD).Trim()
Write-Host "   moro-ai = $MoroCommit"

# Il leggimi del fork viene da questo repository: lo si ricopia a ogni build.
$leggimi = Join-Path $PSScriptRoot 'README.md'
if (Test-Path $leggimi) { Copy-Item $leggimi (Join-Path $script:Repo 'LEGGIMI-MORO.md') -Force }

# ------------------------------------------------------------------ ambiente MSVC

$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (-not (Test-Path $vswhere)) { Fallisci 'vswhere.exe non trovato: servono Visual Studio Build Tools 2022' }
$vs = (& $vswhere -products * -latest -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath | Select-Object -First 1)
if (-not $vs) { Fallisci 'Build Tools senza il carico di lavoro C++ (MSVC x64)' }
$devcmd = Join-Path $vs 'Common7\Tools\VsDevCmd.bat'
Write-Host "-- ambiente MSVC da $vs"
$righe = cmd /c "`"$devcmd`" -arch=x64 -host_arch=x64 -no_logo >nul && set"
foreach ($riga in $righe) {
    if ($riga -match '^([^=]+)=(.*)$') { Set-Item -Path "env:$($Matches[1])" -Value $Matches[2] }
}
foreach ($dir in @('Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin', 'Common7\IDE\CommonExtensions\Microsoft\CMake\Ninja')) {
    $pieno = Join-Path $vs $dir
    if ((Test-Path $pieno) -and ($env:PATH -notlike "*$pieno*")) { $env:PATH = "$pieno;$env:PATH" }
}
foreach ($exe in @('cl', 'cmake', 'ninja', 'git')) {
    if (-not (Get-Command $exe -ErrorAction SilentlyContinue)) { Fallisci "$exe non è nel PATH dopo VsDevCmd" }
}
if (-not $env:VULKAN_SDK) { Fallisci 'VULKAN_SDK non impostata: serve il Vulkan SDK (glslc)' }
$cl = ((cmd /c 'cl 2>&1') | Select-Object -First 1)
$cmakeVer = ((cmake --version) | Select-Object -First 1)
$ninjaVer = (ninja --version)

# ------------------------------------------------------------------ build

$BuildDir = Join-Path $script:Repo "build-moro\$Etichetta"
if ((Test-Path $BuildDir) -and -not $Incrementale) { Remove-Item -Recurse -Force $BuildDir }
$numero = $Tag.Substring(1)
$definizioni = @(
    '-G', 'Ninja',
    '-DCMAKE_BUILD_TYPE=Release',
    '-DGGML_VULKAN=ON',
    '-DGGML_NATIVE=OFF',
    '-DGGML_BACKEND_DL=ON',
    '-DGGML_CPU_ALL_VARIANTS=ON',
    "-DLLAMA_BUILD_NUMBER=$numero",
    '-DLLAMA_BUILD_TESTS=OFF',
    '-DLLAMA_BUILD_EXAMPLES=OFF',
    # L'interfaccia web del server non serve ad Aethera e scaricarla vorrebbe dire un download
    # in più a ogni build: il server risponde alle API anche senza.
    '-DLLAMA_USE_PREBUILT_UI=OFF',
    '-DLLAMA_BUILD_UI=OFF'
)
Write-Host "-- cmake ($cmakeVer, ninja $ninjaVer)"
$tConf = Get-Date
& cmake -S $script:Repo -B $BuildDir @definizioni | Out-Host
if ($LASTEXITCODE -ne 0) { Fallisci "configurazione di cmake fallita (codice $LASTEXITCODE)" }
$durataConf = [int]((Get-Date) - $tConf).TotalSeconds

Write-Host '-- build Release'
$tBuild = Get-Date
& cmake --build $BuildDir --config Release | Out-Host
if ($LASTEXITCODE -ne 0) { Fallisci "build fallita (codice $LASTEXITCODE)" }
$durataBuild = [int]((Get-Date) - $tBuild).TotalSeconds
Write-Host "   configurazione $durataConf s, compilazione $durataBuild s"

# ------------------------------------------------------------------ copia e prova

$bin = Join-Path $BuildDir 'bin'
$server = Join-Path $bin 'llama-server.exe'
if (-not (Test-Path $server)) { Fallisci "la build non ha prodotto $server" }
if (Test-Path $Destinazione) { Remove-Item -Recurse -Force $Destinazione }
New-Item -ItemType Directory -Force $Destinazione | Out-Null
Get-ChildItem $bin -File | Where-Object { $_.Extension -in '.exe', '.dll' } | Copy-Item -Destination $Destinazione
Copy-Item (Join-Path $script:Repo 'LICENSE') $Destinazione

$vecchio = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
$versione = (& (Join-Path $Destinazione 'llama-server.exe') --version 2>&1 | ForEach-Object { "$_" }) -join "`n"
$codiceVersione = $LASTEXITCODE
$ErrorActionPreference = $vecchio
if ($codiceVersione -ne 0 -or $versione -notmatch "build $numero") {
    Fallisci "llama-server --version non risponde come atteso (codice $codiceVersione):`n$versione"
}
$rigaVersione = ($versione -split "`n" | Where-Object { $_ -match '^version:' } | Select-Object -First 1)
Write-Host "   $rigaVersione"

# ------------------------------------------------------------------ provenienza

$fine = Get-Date
$sb = New-Object System.Text.StringBuilder
[void]$sb.AppendLine('# Provenienza di una build locale di llama.cpp (M-14). La scrive .lmbrain-lite/fork/build.ps1;')
[void]$sb.AppendLine('# Aethera la legge e la copia nel manifest di ogni avvio.')
[void]$sb.AppendLine('schema_version = 1')
[void]$sb.AppendLine("id = $(TomlStringa $Id)")
[void]$sb.AppendLine("base = $(TomlStringa $Tag)")
[void]$sb.AppendLine("commit_base = $(TomlStringa $TagCommit)")
[void]$sb.AppendLine("serie = $Serie")
[void]$sb.AppendLine("backend = $(TomlStringa $Backend)")
[void]$sb.AppendLine("ramo = `"moro-ai`"")
[void]$sb.AppendLine("commit = $(TomlStringa $MoroCommit)")
[void]$sb.AppendLine("data = $(TomlStringa ($fine.ToString('yyyy-MM-ddTHH:mm:sszzz')))")
[void]$sb.AppendLine("durata_configurazione_s = $durataConf")
[void]$sb.AppendLine("durata_build_s = $durataBuild")
[void]$sb.AppendLine("durata_totale_s = $([int]($fine - $inizio).TotalSeconds)")
[void]$sb.AppendLine("incrementale = $(if ($Incrementale) { 'true' } else { 'false' })")
[void]$sb.AppendLine("compilatore = $(TomlStringa $cl)")
[void]$sb.AppendLine("cmake = $(TomlStringa $cmakeVer)")
[void]$sb.AppendLine("ninja = $(TomlStringa $ninjaVer)")
[void]$sb.AppendLine("vulkan_sdk = $(TomlStringa (Split-Path $env:VULKAN_SDK -Leaf))")
[void]$sb.AppendLine("opzioni = [$((($definizioni | Where-Object { $_ -like '-D*' }) | ForEach-Object { TomlStringa $_ }) -join ', ')]")
[void]$sb.AppendLine("versione = $(TomlStringa $rigaVersione)")
foreach ($r in $rami) {
    [void]$sb.AppendLine('')
    [void]$sb.AppendLine('[[patch]]')
    [void]$sb.AppendLine("ramo = $(TomlStringa $r.Ramo)")
    [void]$sb.AppendLine("commit = $(TomlStringa $r.Dopo)")
    [void]$sb.AppendLine("commit_prima_del_rebase = $(TomlStringa $r.Prima)")
    [void]$sb.AppendLine("commit_della_patch = [$(($r.Commit | ForEach-Object { TomlStringa $_ }) -join ', ')]")
}
$provenienza = Join-Path $Destinazione 'provenienza.toml'
[System.IO.File]::WriteAllText($provenienza, $sb.ToString(), (New-Object System.Text.UTF8Encoding($false)))
Write-Host "-- provenienza in $provenienza"

# ------------------------------------------------------------------ machine.toml

if (-not $SenzaMachineToml) {
    if (-not (Test-Path $MachineToml)) { Fallisci "$MachineToml non esiste: Aethera non è stato configurato su questa radice" }
    $testo = [System.IO.File]::ReadAllText($MachineToml)
    $giaCe = [regex]::IsMatch($testo, '(?m)^\s*id\s*=\s*"' + [regex]::Escape($Id) + '"\s*$')
    if ($giaCe) {
        Write-Host "-- machine.toml ha già ${Id}: non lo tocco"
    } else {
        $copia = "$MachineToml.prima-di-$($Etichetta -replace '\+','_')-$($fine.ToString('yyyyMMdd-HHmmss'))"
        Copy-Item $MachineToml $copia
        $voce = "`n[[build]]`nid = `"$Id`"`npath = '$Destinazione'`n"
        if (-not $testo.EndsWith("`n")) { $voce = "`n" + $voce }
        [System.IO.File]::AppendAllText($MachineToml, $voce, (New-Object System.Text.UTF8Encoding($false)))
        Write-Host "-- $Id aggiunta a machine.toml (copia di sicurezza: $(Split-Path $copia -Leaf))"
    }
}

Write-Host "== fatto: $Id in $([int]($fine - $inizio).TotalSeconds) s (compilazione $durataBuild s) =="
exit 0
