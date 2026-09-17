"""M-15 — libreria comune della batteria: carica i compiti, prepara una copia di lavoro, verifica.

Nessun percorso della macchina: la batteria si trova accanto a questo file, i risultati vanno sotto
la radice dati di Aethera letta da AETHERA_RADICE (vedi `radice()`).
"""

from __future__ import annotations

import fnmatch
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import time
import tomllib
from pathlib import Path

QUI = Path(__file__).resolve().parent
FIXTURE = QUI / "fixture"
NASCOSTI = QUI / "nascosti"
SOLUZIONI = QUI / "soluzioni"
CONGELATO = QUI / "congelato.json"

# Cartelle che non fanno parte di un repository fixture: prodotti di build, cache, stato di Nonio.
ESCLUSE = {"target", "__pycache__", "node_modules", ".git", ".nonio", ".verifica"}

GITIGNORE = "target/\n__pycache__/\nnode_modules/\n.nonio/\n"


def radice() -> Path:
    r = os.environ.get("AETHERA_RADICE")
    if not r:
        sys.exit(
            "AETHERA_RADICE non è impostata: è la radice dati di Aethera (vedi .lmbrain-lite/percorsi.local.sh "
            "o il README, sezione percorsi). Esempio: set AETHERA_RADICE=<radice>"
        )
    return Path(r)


def carica() -> list[dict]:
    """I compiti con i valori della toolchain e del livello già fusi."""
    with open(QUI / "compiti.toml", "rb") as fh:
        doc = tomllib.load(fh)
    out = []
    for c in doc["compito"]:
        tc = doc["toolchain"][c["toolchain"]]
        lv = doc["livello"][c["livello"]]
        c = dict(c)
        c["verifica"] = list(c.get("verifica", tc["verifica"]))
        c["protetti"] = list(tc["protetti"]) + list(c.get("protetti", []))
        c.setdefault("controlli", [])
        c.setdefault("max_turni", lv["max_turni"])
        c.setdefault("max_secondi", lv["max_secondi"])
        c["enunciato"] = c["enunciato"].strip()
        out.append(c)
    return out


def file_di(cartella: Path) -> list[Path]:
    """File di un albero, relativi, ordinati, senza le cartelle escluse."""
    res = []
    for dirpath, dirnames, filenames in os.walk(cartella):
        dirnames[:] = sorted(d for d in dirnames if d not in ESCLUSE)
        for f in sorted(filenames):
            res.append((Path(dirpath) / f).relative_to(cartella))
    return sorted(res, key=lambda p: p.as_posix())


def contenuto_normale(path: Path) -> bytes:
    """Il contenuto con i fine riga normalizzati: un checkout con autocrlf non cambia gli hash."""
    return path.read_bytes().replace(b"\r\n", b"\n")


def hash_albero(cartella: Path) -> str | None:
    if not cartella.is_dir():
        return None
    h = hashlib.sha256()
    for rel in file_di(cartella):
        h.update(rel.as_posix().encode() + b"\0")
        h.update(hashlib.sha256(contenuto_normale(cartella / rel)).digest())
    return h.hexdigest()


def hash_testo(testo: str) -> str:
    return hashlib.sha256(testo.encode("utf-8")).hexdigest()


def copia_sopra(sorgente: Path, dest: Path) -> list[str]:
    """Copia l'albero `sorgente` sopra `dest`, sovrascrivendo. Restituisce i file copiati."""
    copiati = []
    if not sorgente.is_dir():
        return copiati
    for rel in file_di(sorgente):
        d = dest / rel
        d.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(sorgente / rel, d)
        copiati.append(rel.as_posix())
    return copiati


def prepara(compito: dict, dest: Path) -> None:
    """Una copia pulita del fixture in `dest` (che non deve esistere), con il suo .gitignore."""
    if dest.exists():
        raise FileExistsError(dest)
    dest.mkdir(parents=True)
    copia_sopra(FIXTURE / compito["fixture"], dest)
    (dest / ".gitignore").write_text(GITIGNORE, encoding="utf-8", newline="\n")


def applica_soluzione(compito: dict, dest: Path) -> list[str]:
    return copia_sopra(SOLUZIONI / compito["soluzione"], dest)


def ambiente_verifica() -> dict:
    env = dict(os.environ)
    env["CARGO_NET_OFFLINE"] = "true"
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    env["NO_COLOR"] = "1"
    env["NODE_DISABLE_COLORS"] = "1"
    return env


def _combacia(rel: str, schema: str) -> bool:
    if schema.endswith("/**"):
        return rel.startswith(schema[:-2])
    return fnmatch.fnmatch(rel, schema)


def verifica(compito: dict, lavoro: Path, timeout_s: int = 300) -> dict:
    """Verificatore oggettivo. Non tocca `lavoro`: lavora su una copia accanto.

    1. i file protetti devono essere identici a quelli del fixture (e nessuno cancellato);
    2. i test visibili del fixture vengono rimessi com'erano e quelli nascosti copiati sopra;
    3. i comandi di verifica devono uscire con 0;
    4. i controlli strutturali (regex contate nei sorgenti) devono dare il conteggio atteso.
    """
    fixture = FIXTURE / compito["fixture"]
    esito: dict = {"protetti_violati": [], "comandi": [], "controlli": []}

    for rel in file_di(fixture):
        r = rel.as_posix()
        if any(_combacia(r, s) for s in compito["protetti"]):
            dst = lavoro / rel
            if not dst.is_file():
                esito["protetti_violati"].append(f"{r} (cancellato)")
            elif contenuto_normale(dst) != contenuto_normale(fixture / rel):
                esito["protetti_violati"].append(r)

    copia = lavoro.parent / (lavoro.name + ".verifica")
    if copia.exists():
        shutil.rmtree(copia, ignore_errors=True)
    shutil.copytree(lavoro, copia, ignore=shutil.ignore_patterns(*ESCLUSE))
    for rel in file_di(fixture):
        if rel.parts[0] == "tests":
            (copia / rel).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(fixture / rel, copia / rel)
    copia_sopra(NASCOSTI / compito["fixture"], copia)

    env = ambiente_verifica()
    for cmd in compito["verifica"]:
        t0 = time.monotonic()
        try:
            p = subprocess.run(
                cmd, shell=True, cwd=copia, env=env, capture_output=True, timeout=timeout_s,
                encoding="utf-8", errors="replace",
            )
            codice, uscita = p.returncode, (p.stdout + p.stderr)
        except subprocess.TimeoutExpired as exc:
            codice, uscita = None, f"timeout dopo {timeout_s} s: {exc}"
        esito["comandi"].append({
            "comando": cmd,
            "codice": codice,
            "secondi": round(time.monotonic() - t0, 2),
            "coda": uscita[-1500:],
        })

    for ctl in compito["controlli"]:
        rx = re.compile(ctl["regex"])
        n = 0
        for rel in file_di(copia):
            if fnmatch.fnmatch(rel.as_posix(), ctl["glob"]) or fnmatch.fnmatch(rel.as_posix(), ctl["glob"].replace("**/", "")):
                n += len(rx.findall((copia / rel).read_text(encoding="utf-8", errors="replace")))
        esito["controlli"].append({"glob": ctl["glob"], "regex": ctl["regex"], "atteso": ctl["conteggio"], "trovato": n})

    esito["passato"] = (
        not esito["protetti_violati"]
        and all(c["codice"] == 0 for c in esito["comandi"])
        and all(c["trovato"] == c["atteso"] for c in esito["controlli"])
    )
    shutil.rmtree(copia, ignore_errors=True)
    return esito


def impronte(compito: dict) -> dict:
    return {
        "fixture": hash_albero(FIXTURE / compito["fixture"]),
        "nascosti": hash_albero(NASCOSTI / compito["fixture"]),
        "soluzione": hash_albero(SOLUZIONI / compito["soluzione"]),
        "enunciato": hash_testo(compito["enunciato"]),
        "verifica": hash_testo(json.dumps([compito["verifica"], compito["protetti"], compito["controlli"]], sort_keys=True)),
    }


def controlla_congelato(compiti: list[dict]) -> list[str]:
    """Differenze fra la batteria su disco e `congelato.json`; lista vuota se coincide."""
    if not CONGELATO.is_file():
        return ["congelato.json assente: eseguire congela.py"]
    doc = json.loads(CONGELATO.read_text(encoding="utf-8"))
    diff = []
    for c in compiti:
        atteso = doc["compiti"].get(c["id"])
        if atteso is None:
            diff.append(f"{c['id']}: non congelato")
            continue
        ora = impronte(c)
        for k, v in ora.items():
            if atteso["impronte"].get(k) != v:
                diff.append(f"{c['id']}: {k} cambiato")
    return diff
