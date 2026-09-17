"""M-15 T-06: la prova del riuso del prefisso su Flash-Next a VGM 48, con le stesse regole del runner
della batteria (motore libero, niente build in corso, RAM libera > 16 GiB) e il sorvegliante della RAM.

Uso: python esegui-T06.py [--attendi]      (AETHERA_RADICE obbligatoria; m08_bench compilato)
Scrive <radice>/m15/T-06.jsonl e T-06.meta.json (dal banco), T-06.out e sorveglia-T06.log.
"""

import argparse
import os
import subprocess
import sys
import time
from pathlib import Path

QUI = Path(__file__).resolve().parent
sys.path.insert(0, str(QUI.parent / "batteria"))
import batteria as b  # noqa: E402
import esegui as e  # noqa: E402


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--attendi", action="store_true")
    args = ap.parse_args()
    radice = b.radice()
    out = radice / "m15"
    out.mkdir(parents=True, exist_ok=True)
    repo = Path(os.environ.get("AETHERA_REPO") or QUI.parents[1])
    bench = Path(os.environ.get("AETHERA_M08_BENCH") or repo / "src-tauri" / "target" / "release" / "examples" / "m08_bench.exe")
    if not bench.is_file():
        sys.exit(f"m08_bench non trovato: {bench}")
    while True:
        o = e.ostacoli(radice, e.RAM_MINIMA_FN_GIB)
        if not o:
            break
        if not args.attendi:
            sys.exit("motore non libero: " + "; ".join(o))
        e.log("aspetto: " + "; ".join(o))
        time.sleep(e.SONNO_ATTESA_S)

    stop = radice / "stop-m15-sorveglia"
    stop.unlink(missing_ok=True)
    sorv = subprocess.Popen([
        "powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(QUI.parent / "m10" / "sorveglia-ram.ps1"),
        "-StopFile", str(stop), "-Seconds", "14400", "-MinGiB", "0.8", "-MaxPagefileMB", "4096",
        "-MaxLoadSeconds", "0", "-Log", str(out / "sorveglia-T06.log"),
    ])
    e.log("T-06: avvio del banco")
    with open(out / "T-06.out", "a", encoding="utf-8") as fh:
        codice = subprocess.run([str(bench), str(radice), str(QUI / "T-06-riuso.toml")], stdout=fh, stderr=subprocess.STDOUT).returncode
    stop.write_text("", encoding="utf-8")
    sorv.wait(timeout=60)
    e.log(f"T-06: banco finito con {codice}")
    return codice


if __name__ == "__main__":
    sys.exit(main())
