"""M-15: la sequenza che usa il motore, riprendibile dopo un riavvio.

Fasi, in ordine; ognuna lascia un segno in <radice>/m15/fasi/ e non si ripete:
  attesa     M14-FINITO presente, nessun llama-server/llama-bench, nessuna build, RAM > 16 GiB
  build      cargo build --release degli esempi m15_hold e m08_bench (nel checkout di questo file)
  t04        validazione del runner su G1: fixture intatto e soluzione di riferimento sul motore
             acceso, poi tre compiti veri con Nonio
  g1         batteria completa su G1 (i tre compiti di t04 sono ripresi, non rifatti)
  t06        prova del riuso del prefisso su Flash-Next (esegui-T06.py)
  fc, g3, fn batteria completa sugli altri modelli, in quest'ordine

Uso: python notte.py [--sessione notte-1] [--max-minuti-modello 120] [--fino-a FASE]
Log: <radice>/m15/notte.log (e l'uscita del processo).
"""

import argparse
import datetime as dt
import os
import subprocess
import sys
import time
from pathlib import Path

QUI = Path(__file__).resolve().parent
BATTERIA = QUI.parent / "batteria"
sys.path.insert(0, str(BATTERIA))
import batteria as b  # noqa: E402
import esegui as e  # noqa: E402

VALIDAZIONE = ["rs-durata-en", "py-slug", "ts-giorni"]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--sessione", default="notte-1")
    ap.add_argument("--max-minuti-modello", default="120")
    ap.add_argument("--fino-a")
    args = ap.parse_args()
    radice = b.radice()
    fasi = radice / "m15" / "fasi"
    fasi.mkdir(parents=True, exist_ok=True)
    diario = open(radice / "m15" / "notte.log", "a", encoding="utf-8")

    def scrivi(msg: str) -> None:
        riga = f"[{dt.datetime.now():%Y-%m-%d %H:%M:%S}] {msg}"
        print(riga, flush=True)
        diario.write(riga + "\n")
        diario.flush()

    def runner(*extra: str) -> int:
        cmd = [sys.executable, str(BATTERIA / "esegui.py"), "--max-minuti-modello", args.max_minuti_modello, *extra]
        scrivi("→ " + " ".join(cmd[2:]))
        with open(radice / "m15" / "runner.out", "a", encoding="utf-8") as fh:
            return subprocess.run(cmd, cwd=BATTERIA, stdout=fh, stderr=subprocess.STDOUT).returncode

    def attendi() -> None:
        while True:
            o = e.ostacoli(radice, e.RAM_MINIMA_GIB)
            if not o:
                return
            scrivi("aspetto: " + "; ".join(o))
            time.sleep(e.SONNO_ATTESA_S)

    def fase(nome, fn) -> bool:
        segno = fasi / nome
        if segno.exists():
            scrivi(f"fase {nome}: già fatta ({segno.read_text(encoding='utf-8').strip()})")
            return True
        attendi()
        scrivi(f"fase {nome}: inizio · Windows {e.build_windows()}")
        codice = fn()
        if codice == 0:
            segno.write_text(f"{dt.datetime.now():%Y-%m-%d %H:%M} · Windows {e.build_windows()}\n", encoding="utf-8")
            scrivi(f"fase {nome}: fatta")
        else:
            scrivi(f"fase {nome}: uscita {codice}")
        return codice == 0

    tauri = QUI.parents[1] / "src-tauri"

    def build() -> int:
        cargo = ["cargo", "build", "--release", "--example", "m15_hold", "--example", "m08_bench"]
        with open(radice / "m15" / "build.out", "a", encoding="utf-8") as fh:
            return subprocess.run(cargo, cwd=tauri, stdout=fh, stderr=subprocess.STDOUT).returncode

    os.environ.setdefault("AETHERA_M15_HOLD", str(tauri / "target" / "release" / "examples" / "m15_hold.exe"))
    os.environ.setdefault("AETHERA_M08_BENCH", str(tauri / "target" / "release" / "examples" / "m08_bench.exe"))
    os.environ.setdefault("AETHERA_REPO", str(QUI.parents[1]))
    os.environ.setdefault("AETHERA_NONIO_EXE", os.environ.get("AETHERA_NONIO_EXE", "nonio.exe"))

    comp = [x for c in VALIDAZIONE for x in ("--compito", c)]

    def t04() -> int:
        a = runner("--modello", "G1", "--agente", "nessuno", "--sessione", "t04-validazione", *comp)
        r = runner("--modello", "G1", "--agente", "riferimento", "--sessione", "t04-validazione", *comp)
        n = runner("--modello", "G1", "--sessione", args.sessione, *comp)
        return a or r or n

    def modello(sigla):
        return lambda: runner("--modello", sigla, "--sessione", args.sessione)

    def t06() -> int:
        with open(radice / "m15" / "runner.out", "a", encoding="utf-8") as fh:
            return subprocess.run([sys.executable, str(QUI / "esegui-T06.py")], cwd=QUI, stdout=fh, stderr=subprocess.STDOUT).returncode

    sequenza = [("build", build), ("t04", t04), ("g1", modello("G1")), ("t06", t06),
                ("fc", modello("FC")), ("g3", modello("G3")), ("fn", modello("FN"))]
    esito = 0
    for nome, fn in sequenza:
        if not fase(nome, fn):
            esito = 1
            if nome == "build":
                return 1
        if args.fino_a == nome:
            break
    scrivi("notte finita")
    return esito


if __name__ == "__main__":
    sys.exit(main())
