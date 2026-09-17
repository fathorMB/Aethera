"""M-15 — runner della batteria di coding agentico.

Per ogni modello: aspetta che il motore sia libero, lo accende tramite Aethera (esempio `m15_hold`,
che apre anche l'endpoint 127.0.0.1:8090), e per ogni compito copia il fixture in una cartella
pulita, prende il lock sull'endpoint, esegue `nonio run --json` con i tetti del compito, rilascia
il lock, fa girare il verificatore e scrive una riga JSONL. Alla fine spegne il motore.

Uso (da questa cartella):
  python esegui.py --modello G1 [--modello FC ...] [--compito ID ...] [--ripetizioni 1]
                   [--agente nonio|nessuno|riferimento] [--sessione NOME]
                   [--gia-acceso PRONTO.json] [--max-minuti-modello 150] [--attendi]

Ambiente:
  AETHERA_RADICE     radice dati di Aethera (obbligatoria); i risultati vanno in <radice>/m15
  AETHERA_M15_HOLD   eseguibile di m15_hold (default: <repo>/src-tauri/target/release/examples/m15_hold.exe)
  AETHERA_NONIO_EXE  eseguibile di Nonio (default: nonio.exe dal PATH)

`--agente nessuno` non tocca il fixture e `--agente riferimento` applica la soluzione: servono a
validare il runner (M-15 T-04) con il motore acceso, senza costo di generazione.

Regole del motore (M-15): un avvio alla volta, mai con un altro llama-server acceso, mai mentre
M-14 lavora (`<radice>/m14/M14-FINITO` deve esistere) o una build è in corso, RAM libera > 16 GiB.
Nessuna ricaduta su modelli in cloud: il profilo di Nonio punta solo a 127.0.0.1 e dichiara
l'alias atteso, e ogni riga controlla che le richieste siano arrivate al motore di Aethera.
"""

from __future__ import annotations

import argparse
import ctypes
import datetime as dt
import json
import os
import shutil
import subprocess
import sys
import time
import tomllib
import urllib.error
import urllib.request
from pathlib import Path

import batteria as b

ENDPOINT = "127.0.0.1:8090"
RAM_MINIMA_GIB = 16.0
CTX = 32768
SONNO_ATTESA_S = 300

# Configurazione per modello. `thinking` è quello che Nonio chiede al template (enable_thinking);
# `extra` va alla riga di llama-server (non sono leve gestite dallo schema del profilo);
# `sampling` viene dalla model card (fonte in `fonte_sampling`).
MODELLI = {
    "G1": {
        "profilo": "qwen3.6-35b-a3b.q4_k_m.vulkan",
        "nome": "Qwen3.6-35B-A3B Q4_K_M (G1)",
        "thinking": False,
        "extra": "",
        "max_tokens": 4096,
        "sampling": {"temperature": 0.7, "top_p": 0.8, "top_k": 20, "min_p": 0.0, "presence_penalty": 1.5, "repeat_penalty": 1.0},
        "fonte_sampling": "profilo di Aethera (sampling_by_mode.declared, model card instruct)",
        "ram_minima_gib": RAM_MINIMA_GIB,
        "cache": {},
        "sorveglia_min_gib": None,
    },
    "FC": {
        "profilo": "qwen3.8-flash-coder.q4_k_m.vulkan",
        "nome": "Qwen3.8-Flash-Coder Q4_K_M",
        "thinking": True,
        "extra": '--chat-template-kwargs {"reasoning_effort":"medium"}',
        "max_tokens": 8192,
        "sampling": {"temperature": 1.0, "top_p": 0.95, "top_k": 20, "min_p": 0.0, "presence_penalty": 0.0, "repeat_penalty": 1.0},
        "fonte_sampling": "model card Qwen3.8-Flash-Next (unsloth), modalità thinking, letta il 17-09-2026",
        "ram_minima_gib": RAM_MINIMA_GIB,
        "cache": {},
        "sorveglia_min_gib": None,
    },
    "G3": {
        "profilo": "qwen3-coder-next.q4_k_m.vulkan",
        "nome": "Qwen3-Coder-Next Q4_K_M (G3)",
        "thinking": False,
        "extra": "",
        "max_tokens": 4096,
        "sampling": {"temperature": 1.0, "top_p": 0.95, "top_k": 40, "min_p": 0.01, "repeat_penalty": 1.0},
        "fonte_sampling": "profilo di Aethera (sampling_by_mode.declared)",
        "ram_minima_gib": RAM_MINIMA_GIB,
        "cache": {},
        "sorveglia_min_gib": None,
    },
    "FN": {
        "profilo": "qwen3.8-flash-next.iq3_xxs.vulkan",
        "nome": "Qwen3.8-Flash-Next UD-IQ3_XXS (load_mode none)",
        "thinking": True,
        "extra": '--chat-template-kwargs {"reasoning_effort":"medium"}',
        "max_tokens": 8192,
        "sampling": {"temperature": 1.0, "top_p": 0.95, "top_k": 20, "min_p": 0.0, "presence_penalty": 0.0, "repeat_penalty": 1.0},
        "fonte_sampling": "model card Qwen3.8-Flash-Next (unsloth), modalità thinking, letta il 17-09-2026",
        "ram_minima_gib": RAM_MINIMA_GIB,
        # A VGM 48 restano 2-3 GiB di RAM: niente prompt cache in RAM (un'entrata vale 0,5-0,6 GiB a 7k,
        # M-10 T-09) e pochi checkpoint dello stato ricorrente; il sorvegliante ferma llama-server sotto 0,8 GiB.
        "cache": {"cache_ram": 0, "ctx_checkpoints": 8},
        "sorveglia_min_gib": 0.8,
    },
}


def log(msg: str) -> None:
    print(f"[{dt.datetime.now():%H:%M:%S}] {msg}", flush=True)


# ------------------------------------------------------------------ condizioni della macchina
class _MemStatus(ctypes.Structure):
    _fields_ = [
        ("dwLength", ctypes.c_ulong), ("dwMemoryLoad", ctypes.c_ulong),
        ("ullTotalPhys", ctypes.c_ulonglong), ("ullAvailPhys", ctypes.c_ulonglong),
        ("ullTotalPageFile", ctypes.c_ulonglong), ("ullAvailPageFile", ctypes.c_ulonglong),
        ("ullTotalVirtual", ctypes.c_ulonglong), ("ullAvailVirtual", ctypes.c_ulonglong),
        ("ullAvailExtendedVirtual", ctypes.c_ulonglong),
    ]


def ram_libera_gib() -> float | None:
    try:
        st = _MemStatus()
        st.dwLength = ctypes.sizeof(_MemStatus)
        ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(st))
        return st.ullAvailPhys / 2**30
    except Exception:
        return None


def build_windows() -> str | None:
    """Build di Windows con l'UBR (per esempio 26200.9457): un aggiornamento cambia i numeri."""
    try:
        import winreg

        with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion") as k:
            build = winreg.QueryValueEx(k, "CurrentBuildNumber")[0]
            ubr = winreg.QueryValueEx(k, "UBR")[0]
        return f"{build}.{ubr}"
    except OSError:
        return None


def gia_fatti(jsonl: Path, agente: str) -> set[tuple[str, str, int]]:
    """(modello, compito, ripetizione) già registrati con una riga completa: la ripresa li salta.

    Le righe d'errore d'infrastruttura non contano come fatte, così un riavvio a metà si ripete."""
    fatti = set()
    if not jsonl.is_file():
        return fatti
    for line in jsonl.read_text(encoding="utf-8", errors="replace").splitlines():
        try:
            r = json.loads(line)
        except json.JSONDecodeError:
            continue
        if r.get("compito") and r.get("agente") == agente and r.get("esito") in ("riuscito", "fallito"):
            fatti.add((r["modello"], r["compito"], r["ripetizione"]))
    return fatti


def processi() -> set[str]:
    try:
        out = subprocess.run(["tasklist", "/FO", "CSV", "/NH"], capture_output=True, text=True, errors="replace").stdout
    except OSError:
        return set()
    return {line.split('","')[0].strip('"').lower() for line in out.splitlines() if line.strip()}


def ostacoli(radice: Path, ram_minima: float) -> list[str]:
    o = []
    if not (radice / "m14" / "M14-FINITO").is_file():
        o.append("M-14 non ha finito (manca <radice>/m14/M14-FINITO)")
    p = processi()
    for occupante in ("llama-server.exe", "llama-bench.exe", "m08_bench.exe", "m15_hold.exe"):
        if occupante in p:
            o.append(f"{occupante} è acceso")
    build = sorted(p & {"cmake.exe", "ninja.exe", "cl.exe"})
    if build:
        o.append(f"build in corso: {', '.join(build)}")
    ram = ram_libera_gib()
    if ram is None or ram < ram_minima:
        o.append(f"RAM libera {ram if ram is None else round(ram, 1)} GiB sotto {ram_minima}")
    return o


# ------------------------------------------------------------------ endpoint di Aethera
def http(metodo: str, percorso: str, corpo: dict | None = None, timeout: float = 30) -> tuple[int, dict]:
    dati = json.dumps(corpo).encode() if corpo is not None else None
    req = urllib.request.Request(f"http://{ENDPOINT}{percorso}", data=dati, method=metodo)
    if dati is not None:
        req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return r.status, json.loads(r.read() or b"{}")
    except urllib.error.HTTPError as e:
        try:
            return e.code, json.loads(e.read() or b"{}")
        except Exception:
            return e.code, {}
    except (urllib.error.URLError, OSError) as e:
        return 0, {"error": str(e)}


def telemetria() -> tuple[list[dict], dict]:
    st, doc = http("GET", "/telemetry/recent?n=1000")
    if st != 200:
        return [], {}
    return doc.get("records", []), doc.get("summary", {})


# ------------------------------------------------------------------ motore
class Motore:
    def __init__(self, radice: Path, sigla: str, cartella: Path):
        self.radice = radice
        self.cfg = MODELLI[sigla]
        self.sigla = sigla
        self.cartella = cartella
        self.proc: subprocess.Popen | None = None
        self.pronto: dict = {}

    def avvia(self, timeout_s: int = 1800) -> dict:
        exe = os.environ.get("AETHERA_M15_HOLD")
        if not exe:
            repo = os.environ.get("AETHERA_REPO") or str(b.QUI.parents[1])
            exe = str(Path(repo) / "src-tauri" / "target" / "release" / "examples" / "m15_hold.exe")
        if not Path(exe).is_file():
            raise RuntimeError(f"m15_hold non trovato: {exe} (cargo build --release --example m15_hold)")
        pronto = self.cartella / f"pronto-{self.sigla}.json"
        pronto.unlink(missing_ok=True)
        cmd = [exe, str(self.radice), self.cfg["profilo"], "--ctx", str(CTX), "--pronto", str(pronto)]
        if self.cfg["extra"]:
            cmd += ["--extra", self.cfg["extra"]]
        for leva, valore in self.cfg["cache"].items():
            cmd += ["--" + leva.replace("_", "-"), str(valore)]
        self.sorvegliante = None
        if self.cfg["sorveglia_min_gib"] is not None:
            script = b.QUI.parent / "m10" / "sorveglia-ram.ps1"
            (self.radice / "stop-m15-sorveglia").unlink(missing_ok=True)
            self.sorvegliante = subprocess.Popen([
                "powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(script),
                "-StopFile", str(self.radice / "stop-m15-sorveglia"), "-Seconds", "43200",
                "-MinGiB", str(self.cfg["sorveglia_min_gib"]), "-MaxPagefileMB", "4096", "-MaxLoadSeconds", "0",
                "-Log", str(self.cartella / f"sorveglia-{self.sigla}.log"),
            ])
        self.log_hold = open(self.cartella / f"hold-{self.sigla}.log", "a", encoding="utf-8")
        log(f"avvio {self.cfg['profilo']} tramite m15_hold")
        self.proc = subprocess.Popen(cmd, stdout=self.log_hold, stderr=subprocess.STDOUT)
        t0 = time.monotonic()
        while not pronto.is_file():
            if self.proc.poll() is not None:
                raise RuntimeError(f"m15_hold è uscito con {self.proc.returncode} prima di essere pronto (vedi hold-{self.sigla}.log)")
            if time.monotonic() - t0 > timeout_s:
                self.ferma()
                raise RuntimeError("il motore non è arrivato a pronto")
            time.sleep(2)
        time.sleep(0.5)
        self.pronto = json.loads(pronto.read_text(encoding="utf-8"))
        log(f"pronto: run {self.pronto['run_id']} · {self.pronto['alias']} · ctx {self.pronto['ctx_served']} · caricamento {self.pronto['load_ms'] / 1000:.0f} s · RAM libera {ram_libera_gib():.1f} GiB")
        return self.pronto

    def ferma(self) -> None:
        if not self.proc:
            return
        (self.radice / "stop-m15").write_text("", encoding="utf-8")
        try:
            self.proc.wait(timeout=240)
        except subprocess.TimeoutExpired:
            log("m15_hold non si ferma: lo chiudo (il job object chiude llama-server)")
            subprocess.run(["taskkill", "/T", "/F", "/PID", str(self.proc.pid)], capture_output=True)
            self.proc.wait(timeout=60)
        log(f"motore spento (m15_hold uscito con {self.proc.returncode})")
        if self.sorvegliante:
            (self.radice / "stop-m15-sorveglia").write_text("", encoding="utf-8")
            try:
                self.sorvegliante.wait(timeout=30)
            except subprocess.TimeoutExpired:
                self.sorvegliante.kill()
        self.proc = None
        self.log_hold.close()


# ------------------------------------------------------------------ Nonio
def profilo_nonio(cfg: dict, pronto: dict, dest: Path) -> Path:
    base = pronto["base_url"]
    if not base.startswith("http://127.0.0.1:"):
        raise RuntimeError(f"base_url non locale: {base}")
    righe = [
        f'name = "m15-{cfg["profilo"]}"',
        "",
        "[backend]",
        'kind = "llamacpp"',
        f'base_url = "{base}"',
        "timeout_s = 900",
        "",
        "[family]",
        'kind = "qwen"',
        f"thinking = {'true' if cfg['thinking'] else 'false'}",
        "",
        "[model]",
        f'expected = "{pronto["alias"]}"',
        "",
        "[context]",
        f"declared = {pronto.get('ctx_served') or CTX}",
        "",
        "[sampling]",
    ]
    for k, v in cfg["sampling"].items():
        righe.append(f"{k} = {v}")
    righe.append(f"max_tokens = {cfg['max_tokens']}")
    dest.write_text("\n".join(righe) + "\n", encoding="utf-8", newline="\n")
    return dest


def nonio_toml(compito: dict, cfg: dict, ctx: int) -> str:
    # Budget: segmenti fissi misurati da Nonio (~2,7k) con margine; la conversazione prende il resto
    # del contesto meno l'output massimo del turno.
    fissi = 1500 + 3200 + 2000 + 500 + 1500
    conversazione = max(4000, ctx - cfg["max_tokens"] - fissi)
    cmds = ", ".join(json.dumps(c) for c in compito["verifica"])
    return (
        "# Scritto dal runner di M-15: verifica visibile, tetti del compito, niente rete.\n"
        f"[verify]\ncommands = [{cmds}]\ntimeout_s = 300\n\n"
        "[tools]\nallow_network = false\n\n"
        f"[budgets]\nsystem = 1500\ntools = 3200\nrepo_map = 2000\nmemory = 500\ntask = 1500\nconversation = {conversazione}\n\n"
        '[repo_map]\nexclude = ["target", "node_modules", "__pycache__"]\nmax_files = 200\n\n'
        f"[limits]\nmax_turns = {compito['max_turni']}\nwallclock_s = {compito['max_secondi']}\n"
    )


def git(ws: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git", "-c", "user.name=m15", "-c", "user.email=m15@localhost", "-c", "core.autocrlf=false", *args],
        cwd=ws, capture_output=True, text=True, errors="replace",
    )


def prepara_lavoro(compito: dict, cfg: dict, ws: Path, ctx: int) -> None:
    b.prepara(compito, ws)
    (ws / "nonio.toml").write_text(nonio_toml(compito, cfg, ctx), encoding="utf-8", newline="\n")
    git(ws, "init", "-q")
    git(ws, "add", "-A")
    git(ws, "commit", "-q", "-m", "fixture della batteria M-15")


def file_toccati(ws: Path) -> list[str]:
    out = git(ws, "status", "--porcelain", "--untracked-files=all").stdout
    return sorted(line[3:].strip() for line in out.splitlines() if line.strip())


def esegui_nonio(compito: dict, profilo: Path, ws: Path, cartella: Path) -> dict:
    exe = os.environ.get("AETHERA_NONIO_EXE", "nonio.exe")
    cmd = [
        exe, "run", "--json",
        "--max-turns", str(compito["max_turni"]),
        "--budget-wallclock", str(compito["max_secondi"]),
        "-p", str(profilo), "-w", str(ws),
        compito["enunciato"],
    ]
    uscita = cartella / "nonio-stdout.jsonl"
    errori = cartella / "nonio-stderr.txt"
    t0 = time.monotonic()
    with open(uscita, "wb") as fo, open(errori, "wb") as fe:
        p = subprocess.Popen(cmd, stdout=fo, stderr=fe, cwd=ws)
        try:
            codice = p.wait(timeout=compito["max_secondi"] + 240)
            ucciso = False
        except subprocess.TimeoutExpired:
            subprocess.run(["taskkill", "/T", "/F", "/PID", str(p.pid)], capture_output=True)
            codice = p.wait(timeout=60)
            ucciso = True
    return {"codice": codice, "secondi": round(time.monotonic() - t0, 1), "ucciso_dal_runner": ucciso,
            "stdout": uscita, "stderr": errori}


def _somma(valori: list) -> int | None:
    """Somma dichiarata sconosciuta (None) se manca anche un solo valore: assente non è zero."""
    if not valori or any(v is None for v in valori):
        return None
    return int(sum(valori))


def leggi_traccia(percorso: Path) -> dict:
    eventi = []
    for line in percorso.read_text(encoding="utf-8", errors="replace").splitlines():
        line = line.strip()
        if line.startswith("{"):
            try:
                eventi.append(json.loads(line))
            except json.JSONDecodeError:
                pass
    risposte = [e for e in eventi if e.get("event") == "response_complete"]
    acc = [e.get("accounting", {}) for e in risposte]
    chiamate = [e for e in eventi if e.get("event") == "tool_called"]
    risultati = [e for e in eventi if e.get("event") == "tool_result"]
    falliti = [r for r in risultati if (r.get("result", {}).get("outcome", {}).get("status") != "ok")]
    per_tipo: dict[str, int] = {}
    for r in falliti:
        k = r["result"]["outcome"].get("kind", "?")
        per_tipo[k] = per_tipo.get(k, 0) + 1
    per_strumento: dict[str, int] = {}
    for c in chiamate:
        n = c.get("call", {}).get("name", "?")
        per_strumento[n] = per_strumento.get(n, 0) + 1
    aperture = [e for e in eventi if e.get("event") == "session_open"]
    chiusure = [e for e in eventi if e.get("event") == "session_close"]
    verifiche = [e for e in eventi if e.get("event") == "verification_run"]
    sessioni = sorted({e.get("session_id") for e in eventi if e.get("session_id")})
    compattazioni = sum(1 for e in eventi if e.get("event") == "forked") or sum(1 for e in chiusure if e.get("continues_in_fork"))
    prompt = [a.get("prompt_tokens") for a in acc]
    cached = [a.get("cached_tokens") for a in acc]

    # Riassunto leggibile: una riga per chiamata fallita e per fine sessione, poche righe in tutto.
    riassunto = []
    nomi = {c.get("call", {}).get("id"): c.get("call", {}) for c in chiamate}
    for r in risultati:
        out = r.get("result", {}).get("outcome", {})
        call = nomi.get(r.get("result", {}).get("call_id"), {})
        arg = json.dumps(call.get("arguments", {}), ensure_ascii=False)[:90]
        if out.get("status") != "ok":
            riassunto.append(f"t{r.get('turn_index')} {call.get('name')}({arg}) -> {out.get('kind')}: {str(out.get('message', ''))[:160]}")
    for e in verifiche:
        riassunto.append(f"verifica di Nonio: {e.get('command')} -> {e.get('outcome')} ({e.get('exit_code')})")
    for e in chiusure:
        riassunto.append(f"fine: {str(e.get('stop_reason', ''))[:200]} · turni {e.get('turns')}")
    ultimo_testo = ""
    for e in eventi:
        if e.get("event") == "stream_chunk" and e.get("channel") == "content":
            ultimo_testo = e.get("text", "")
    tronchi = sum(1 for e in risposte if str(e.get("finish_reason", "")).lower() in ("length", "max_tokens"))

    return {
        "eventi": len(eventi),
        "sessioni": sessioni,
        "modelli_serviti": sorted({(a.get("engine") or {}).get("model_alias") for a in aperture if a.get("engine")} - {None}),
        "build_servite": sorted({(a.get("engine") or {}).get("build_info") for a in aperture if a.get("engine")} - {None}),
        "capacita": (aperture[0].get("capabilities") if aperture else None),
        "turni": len(risposte) if risposte else (0 if aperture else None),
        "turni_dichiarati": (sum(e.get("turns") or 0 for e in chiusure) if chiusure else None),
        "stop_reason": (chiusure[-1].get("stop_reason") if chiusure else None),
        "durata_ms_nonio": _somma([e.get("duration_ms") for e in chiusure]) if chiusure else None,
        "token_prompt": _somma(prompt),
        "token_prompt_max": (max(prompt) if prompt and None not in prompt else None),
        "token_cache": _somma(cached),
        "token_elaborati": (_somma(prompt) - _somma(cached)) if (_somma(prompt) is not None and _somma(cached) is not None) else None,
        "token_output": _somma([a.get("content_tokens") for a in acc]),
        "token_strumenti": _somma([a.get("tool_tokens") for a in acc]),
        "token_ragionamento": _somma([a.get("reasoning_tokens") for a in acc]),
        "prefill_ms": _somma([a.get("prefill_ms") for a in acc]),
        "decode_ms": _somma([a.get("decode_ms") for a in acc]),
        "turni_troncati": tronchi,
        "chiamate": len(chiamate),
        "chiamate_per_strumento": per_strumento,
        "chiamate_fallite": len(falliti),
        "fallite_per_tipo": per_tipo,
        "compattazioni": compattazioni,
        "verifiche_nonio": [{"comando": e.get("command"), "esito": e.get("outcome"), "codice": e.get("exit_code")} for e in verifiche],
        "ultimo_testo": ultimo_testo[-400:],
        "riassunto": riassunto[-25:],
    }


def causa(riga: dict) -> str | None:
    if riga["verifica_passata"]:
        return None
    v = riga["verifica"]
    if riga.get("errore_infrastruttura"):
        return "infrastruttura"
    if v.get("protetti_violati"):
        return "file_protetti"
    sr = (riga.get("stop_reason") or "").lower()
    if riga.get("nonio_codice") == 5:
        return "budget_rifiutato"
    if riga.get("nonio_codice") == 3:
        return "infrastruttura"
    if riga.get("ucciso_dal_runner"):
        return "tetto_tempo"
    if "turn ceiling" in sr:
        return "tetto_turni"
    if "time ceiling" in sr:
        return "tetto_tempo"
    if "conversation reached" in sr:
        return "contesto_pieno"
    if "output ceiling" in sr:
        return "output_troncato"
    if not riga.get("file_toccati_sorgenti"):
        return "nessuna_modifica"
    if any(c["trovato"] != c["atteso"] for c in v.get("controlli", [])):
        return "controllo_strutturale"
    return "test_falliti"


# ------------------------------------------------------------------ un compito
def esegui_compito(compito: dict, sigla: str, cfg: dict, pronto: dict | None, profilo: Path | None,
                   agente: str, rip: int, cartelle: dict, sessione: str, congelato_ok: bool) -> dict:
    nome = f"{compito['id']}-r{rip}"
    ws = cartelle["lavoro"] / sigla / nome
    art = cartelle["risultati"] / sigla / nome
    if ws.exists():
        shutil.rmtree(ws, ignore_errors=True)
    art.mkdir(parents=True, exist_ok=True)
    ctx = (pronto or {}).get("ctx_served") or CTX
    prepara_lavoro(compito, cfg, ws, ctx)

    riga: dict = {
        "sessione": sessione,
        "compito": compito["id"],
        "fixture": compito["fixture"],
        "lingua": compito["lingua"],
        "livello": compito["livello"],
        "tipo": compito["tipo"],
        "toolchain": compito["toolchain"],
        "ripetizione": rip,
        "modello": sigla,
        "profilo": cfg["profilo"],
        "thinking": cfg["thinking"],
        "extra_motore": cfg["extra"] or None,
        "cache_motore": cfg["cache"] or None,
        "agente": agente,
        "tetti": {"max_turni": compito["max_turni"], "max_secondi": compito["max_secondi"]},
        "batteria_congelata": congelato_ok,
        "run_id": (pronto or {}).get("run_id"),
        "build": (pronto or {}).get("build"),
        "ctx_servito": (pronto or {}).get("ctx_served"),
        "inizio": dt.datetime.now().isoformat(timespec="seconds"),
        "windows": build_windows(),
        "ram_libera_gib_inizio": round(ram_libera_gib() or 0, 1) or None,
    }

    rec_prima, _ = telemetria() if pronto else ([], {})
    ultimo_task = max((r.get("task", -1) for r in rec_prima), default=-1)
    t0 = time.monotonic()
    esito_nonio = None
    lock_ok = None
    if agente == "nonio":
        st, doc = http("POST", "/lock", {"client": "nonio", "label": f"m15 {nome}", "ttl_s": compito["max_secondi"] + 600})
        lock_ok = st == 201
        if not lock_ok:
            riga["errore_infrastruttura"] = f"lock rifiutato: HTTP {st} {doc}"
        else:
            try:
                esito_nonio = esegui_nonio(compito, profilo, ws, art)
            finally:
                time.sleep(2.5)  # le ultime richieste entrano nella telemetria al giro dopo del monitor
                http("DELETE", "/lock?client=nonio")
    elif agente == "riferimento":
        b.applica_soluzione(compito, ws)
    riga["secondi"] = round(time.monotonic() - t0, 1)
    riga["lock_preso"] = lock_ok

    # Traccia di Nonio.
    if esito_nonio:
        riga["nonio_codice"] = esito_nonio["codice"]
        riga["ucciso_dal_runner"] = esito_nonio["ucciso_dal_runner"]
        tr = leggi_traccia(esito_nonio["stdout"])
        riga.update({k: v for k, v in tr.items() if k not in ("ultimo_testo", "riassunto", "capacita")})
        (art / "riassunto.json").write_text(json.dumps(
            {"riassunto": tr["riassunto"], "ultimo_testo": tr["ultimo_testo"], "capacita": tr["capacita"],
             "stderr_coda": esito_nonio["stderr"].read_text(encoding="utf-8", errors="replace")[-3000:]},
            indent=2, ensure_ascii=False), encoding="utf-8")
        riga["riassunto"] = tr["riassunto"][-8:]
        if tr["eventi"] == 0:
            coda = esito_nonio["stderr"].read_text(encoding="utf-8", errors="replace")[-400:]
            riga["errore_infrastruttura"] = f"Nonio non ha scritto eventi (uscita {esito_nonio['codice']}): {coda}"
    else:
        for k in ("nonio_codice", "ucciso_dal_runner", "turni", "stop_reason", "token_prompt", "token_cache",
                  "token_elaborati", "token_output", "token_strumenti", "token_ragionamento", "prefill_ms",
                  "decode_ms", "chiamate", "chiamate_fallite", "compattazioni", "modelli_serviti"):
            riga[k] = None

    # Lato motore: le richieste arrivate ad Aethera durante il compito.
    if pronto:
        rec_dopo, summ = telemetria()
        nuovi = [r for r in rec_dopo if r.get("task", -1) > ultimo_task]
        ids = {r.get("task") for r in nuovi}
        riga["motore_richieste"] = len(nuovi)
        riga["motore_client"] = sorted({r.get("client") or "?" for r in nuovi})
        riga["motore_prompt_elaborati"] = sum(r.get("prompt_n", 0) for r in nuovi) if nuovi else (0 if agente != "nonio" else None)
        riga["motore_cache"] = _somma([r.get("cache_n") for r in nuovi]) if nuovi else None
        riga["motore_generati"] = sum(r.get("gen_n", 0) for r in nuovi) if nuovi else None
        riga["motore_prefill_s"] = round(sum(r.get("prompt_ms", 0) for r in nuovi) / 1000, 1) if nuovi else None
        riga["motore_decode_s"] = round(sum(r.get("gen_ms", 0) for r in nuovi) / 1000, 1) if nuovi else None
        dec = [r["decode_tps"] for r in nuovi if r.get("decode_tps")]
        riga["motore_decode_tps_mediana"] = round(sorted(dec)[len(dec) // 2], 2) if dec else None
        riga["motore_compattazioni"] = sum(1 for c in summ.get("compactions", []) if set(c.get("tasks", [])) & ids)
    else:
        for k in ("motore_richieste", "motore_client", "motore_prompt_elaborati", "motore_cache", "motore_generati",
                  "motore_prefill_s", "motore_decode_s", "motore_decode_tps_mediana", "motore_compattazioni"):
            riga[k] = None

    # Anti-ricaduta sul cloud: il modello che ha risposto è quello acceso da Aethera, e le risposte
    # contate da Nonio sono arrivate tutte al motore locale.
    if agente == "nonio" and pronto:
        risposte = riga.get("turni") or 0
        riga["solo_motore_locale"] = bool(
            riga.get("modelli_serviti") == [pronto["alias"]]
            and riga.get("motore_richieste") is not None
            and riga["motore_richieste"] >= risposte
            and set(riga.get("motore_client") or []) <= {"nonio"}
        ) if risposte else (riga.get("motore_richieste") == 0)
    else:
        riga["solo_motore_locale"] = None

    toccati = file_toccati(ws)
    riga["file_toccati"] = toccati
    riga["file_toccati_sorgenti"] = [f for f in toccati if f not in ("Cargo.lock",) and not f.startswith(".")]
    v = b.verifica(compito, ws)
    riga["verifica"] = {
        "protetti_violati": v["protetti_violati"],
        "comandi": [{"comando": c["comando"], "codice": c["codice"], "secondi": c["secondi"]} for c in v["comandi"]],
        "controlli": v["controlli"],
    }
    riga["verifica_passata"] = v["passato"]
    riga["esito"] = "riuscito" if v["passato"] else ("errore" if riga.get("errore_infrastruttura") else "fallito")
    riga["causa"] = causa(riga)
    (art / "verifica.json").write_text(json.dumps(v, indent=2, ensure_ascii=False), encoding="utf-8")
    return riga


# ------------------------------------------------------------------ main
def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--modello", action="append", choices=list(MODELLI), required=True)
    ap.add_argument("--compito", action="append")
    ap.add_argument("--ripetizioni", type=int, default=1)
    ap.add_argument("--agente", choices=["nonio", "nessuno", "riferimento"], default="nonio")
    ap.add_argument("--sessione")
    ap.add_argument("--gia-acceso", help="pronto.json di un m15_hold già acceso: il runner non avvia né spegne")
    ap.add_argument("--senza-motore", action="store_true", help="solo per --agente nessuno/riferimento: non avvia nulla")
    ap.add_argument("--max-minuti-modello", type=float, default=150.0)
    ap.add_argument("--attendi", action="store_true", help="aspetta che il motore sia libero invece di fermarsi")
    args = ap.parse_args()

    radice = b.radice()
    compiti = b.carica()
    if args.compito:
        sconosciuti = set(args.compito) - {c["id"] for c in compiti}
        if sconosciuti:
            sys.exit(f"compiti sconosciuti: {sorted(sconosciuti)}")
        compiti = [c for c in compiti if c["id"] in args.compito]
    diff = b.controlla_congelato(compiti)
    congelato_ok = not diff
    if diff:
        log("ATTENZIONE: la batteria non coincide con congelato.json: " + "; ".join(diff))
    if args.agente == "nonio" and args.senza_motore:
        sys.exit("--senza-motore vale solo per --agente nessuno o riferimento")

    sessione = args.sessione or dt.datetime.now().strftime("s-%Y%m%d-%H%M")
    cartelle = {
        "lavoro": radice / "m15" / "lavoro" / sessione,
        "risultati": radice / "m15" / "risultati" / sessione,
    }
    for c in cartelle.values():
        c.mkdir(parents=True, exist_ok=True)
    jsonl = cartelle["risultati"] / "risultati.jsonl"
    log(f"sessione {sessione}: {len(compiti)} compiti × {args.ripetizioni} × modelli {args.modello} → {jsonl}")

    log(f"Windows {build_windows()}")
    esito_globale = 0
    for sigla in args.modello:
        cfg = MODELLI[sigla]
        fatti = gia_fatti(jsonl, args.agente)
        da_fare = [(rip, c) for rip in range(1, args.ripetizioni + 1) for c in compiti if (sigla, c["id"], rip) not in fatti]
        if not da_fare:
            log(f"{sigla}: tutti i compiti sono già nel JSONL, salto il modello")
            continue
        if len(da_fare) < len(compiti) * args.ripetizioni:
            log(f"{sigla}: ripresa, {len(compiti) * args.ripetizioni - len(da_fare)} compiti già fatti, ne restano {len(da_fare)}")
        motore = None
        pronto = None
        profilo = None
        try:
            if args.gia_acceso:
                pronto = json.loads(Path(args.gia_acceso).read_text(encoding="utf-8"))
                if pronto["alias"] != cfg["profilo"]:
                    raise RuntimeError(f"il motore acceso serve {pronto['alias']}, non {cfg['profilo']}")
            elif not args.senza_motore:
                while True:
                    o = ostacoli(radice, cfg["ram_minima_gib"])
                    if not o:
                        break
                    if not args.attendi:
                        raise RuntimeError("motore non libero: " + "; ".join(o))
                    log("aspetto: " + "; ".join(o))
                    time.sleep(SONNO_ATTESA_S)
                motore = Motore(radice, sigla, cartelle["risultati"])
                pronto = motore.avvia()
            if pronto:
                profilo = profilo_nonio(cfg, pronto, cartelle["risultati"] / f"nonio-{sigla}.toml")
                st, _ = http("GET", "/status")
                if st != 200:
                    raise RuntimeError(f"endpoint di Aethera non risponde su {ENDPOINT} (HTTP {st})")

            t_modello = time.monotonic()
            for rip, c in da_fare:
                minuti = (time.monotonic() - t_modello) / 60
                if minuti > args.max_minuti_modello:
                    log(f"{sigla}: superato il tempo massimo di {args.max_minuti_modello} min, interrompo il modello")
                    with open(jsonl, "a", encoding="utf-8") as fh:
                        fh.write(json.dumps({"sessione": sessione, "modello": sigla, "evento": "modello_interrotto",
                                             "minuti": round(minuti, 1), "compito_saltato_da": c["id"], "ripetizione": rip},
                                            ensure_ascii=False) + "\n")
                    raise StopIteration
                if motore and motore.proc and motore.proc.poll() is not None:
                    raise RuntimeError("il motore si è spento durante la batteria")
                log(f"{sigla} · {c['id']} r{rip} ({c['livello']}, {c['max_turni']} turni / {c['max_secondi']} s)")
                riga = esegui_compito(c, sigla, cfg, pronto, profilo, args.agente, rip, cartelle, sessione, congelato_ok)
                with open(jsonl, "a", encoding="utf-8") as fh:
                    fh.write(json.dumps(riga, ensure_ascii=False) + "\n")
                log(f"   → {riga['esito']}{' (' + riga['causa'] + ')' if riga['causa'] else ''} · {riga['secondi']} s · "
                    f"turni {riga.get('turni')} · richieste al motore {riga.get('motore_richieste')} · solo locale {riga.get('solo_motore_locale')}")
                if args.agente == "nonio" and riga.get("solo_motore_locale") is False:
                    raise RuntimeError("una richiesta non risulta arrivata al motore locale: mi fermo")
        except StopIteration:
            pass
        except Exception as exc:  # noqa: BLE001 — il motore va spento comunque
            log(f"{sigla}: ERRORE {exc}")
            esito_globale = 1
            with open(jsonl, "a", encoding="utf-8") as fh:
                fh.write(json.dumps({"sessione": sessione, "modello": sigla, "evento": "errore", "messaggio": str(exc)},
                                    ensure_ascii=False) + "\n")
        finally:
            if motore:
                motore.ferma()
    return esito_globale


if __name__ == "__main__":
    sys.exit(main())
