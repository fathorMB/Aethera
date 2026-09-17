"""M-16 T-03: la batteria di M-15 sui profili di prova, riprendibile dopo un riavvio.

Fasi (segni in <radice>/m16/fasi/, come notte.py di M-15):
  g3-int8    G3 su b10991+moro1 con l'ubatch scelto da T-02b (profilo qwen3-coder-next.q4_k_m.vulkan.int8)
  g3-moro0   controllo: G3 su b10991+moro0 con lo stesso ubatch (profilo ….moro0). notte-1 girava su
             b10809 e ubatch 512: senza questo controllo la patch non si separa da tag e ubatch.
  g1-int8    solo con --g1: G1 su b10991+moro1 (profilo qwen3.6-35b-a3b.q4_k_m.vulkan.int8)

Stessi tetti di notte-1 (quelli di compiti.toml), sessione nuova (int8-1), un giro per modello.
Ogni fase aspetta che il motore sia libero (esegui.py --attendi: nessun llama-server, nessuna build,
RAM libera > 16 GiB).

Uso: python batteria.py [--sessione int8-1] [--g1] [--max-minuti-modello 120]
Ambiente: AETHERA_RADICE; AETHERA_NONIO_EXE (Nonio, come in notte-1); AETHERA_M15_HOLD facoltativo.
"""

import argparse
import datetime as dt
import os
import subprocess
import sys
from pathlib import Path

QUI = Path(__file__).resolve().parent
BATTERIA = QUI.parent / "batteria"
sys.path.insert(0, str(BATTERIA))
import batteria as b  # noqa: E402
import esegui as e  # noqa: E402


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--sessione", default="int8-1")
    ap.add_argument("--g1", action="store_true")
    ap.add_argument("--max-minuti-modello", default="120")
    args = ap.parse_args()
    radice = b.radice()
    fasi = radice / "m16" / "fasi"
    fasi.mkdir(parents=True, exist_ok=True)
    diario = open(radice / "m16" / "batteria.log", "a", encoding="utf-8")

    def scrivi(msg: str) -> None:
        riga = f"[{dt.datetime.now():%Y-%m-%d %H:%M:%S}] {msg}"
        print(riga, flush=True)
        diario.write(riga + "\n")
        diario.flush()

    tauri = QUI.parents[1] / "src-tauri"
    os.environ.setdefault("AETHERA_M15_HOLD", str(tauri / "target" / "release" / "examples" / "m15_hold.exe"))
    os.environ.setdefault("AETHERA_REPO", str(QUI.parents[1]))

    # La sequenza di T-01/T-02 usa anche llama-perplexity, che esegui.py non conosce: si aspetta
    # che abbia finito (segno di t02-g3) e che nessun llama-perplexity sia acceso.
    import time

    while not (fasi / "t02-g3").exists() or "llama-perplexity.exe" in e.processi():
        scrivi("aspetto la fine di sequenza.py (T-01, T-02)")
        time.sleep(120)

    sequenza = [("g3-int8", "G3-int8"), ("g3-moro0", "G3-moro0")] + ([("g1-int8", "G1-int8")] if args.g1 else [])
    esito = 0
    for nome, sigla in sequenza:
        segno = fasi / f"batteria-{nome}"
        if segno.exists():
            scrivi(f"fase {nome}: già fatta ({segno.read_text(encoding='utf-8').strip()})")
            continue
        scrivi(f"fase {nome}: inizio · Windows {e.build_windows()}")
        t0 = dt.datetime.now()
        cmd = [sys.executable, str(BATTERIA / "esegui.py"), "--modello", sigla, "--sessione", args.sessione,
               "--attendi", "--max-minuti-modello", args.max_minuti_modello]
        with open(radice / "m16" / "batteria-runner.out", "a", encoding="utf-8") as fh:
            codice = subprocess.run(cmd, cwd=BATTERIA, stdout=fh, stderr=subprocess.STDOUT).returncode
        minuti = (dt.datetime.now() - t0).total_seconds() / 60
        if codice == 0:
            segno.write_text(f"{dt.datetime.now():%Y-%m-%d %H:%M} · {minuti:.0f} min · Windows {e.build_windows()}\n", encoding="utf-8")
            scrivi(f"fase {nome}: fatta in {minuti:.0f} min")
        else:
            scrivi(f"fase {nome}: uscita {codice} dopo {minuti:.0f} min")
            esito = 1
    scrivi("batteria finita")
    return esito


if __name__ == "__main__":
    sys.exit(main())
