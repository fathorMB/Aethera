"""M-16: le misure che usano il motore, in ordine e riprendibili dopo un riavvio.

Fasi (ognuna lascia un segno in <radice>/m16/fasi/ e non si ripete):
  kld-g1, kld-g3   T-01: llama-perplexity con --kl-divergence-base sulla base moro0 all'ubatch del
                   profilo, poi --kl-divergence per: la stessa base ripetuta (rumore), la base con un
                   altro ubatch (metro), la patch moro1 all'ubatch del profilo e all'altro ubatch
  picco            T-01, indagine: i logit di moro1 e della base con l'altro ubatch salvati anche
                   loro come file «base», poi kld_per_token.py token per token contro la base
                   (dove cade il massimo di KLD, se è un token solo o un gruppo, che token è)
  t02-g1, t02-g3   T-02: m08_bench con gli scenari T-02a.toml e T-02b.toml (--riprendi)

Opzioni di llama-perplexity come run-T06-perplexity.sh di M-14: prompt congelato da 21k, contesto
8192, -ngl 999, flash attention, cache f16; ubatch e batch come i profili (G1 4096/4096, G3 512/2048).

Prima di ogni avvio: nessun llama-server/llama-bench/llama-perplexity/banco acceso, nessuna build,
RAM libera sopra 16 GiB (le regole di M-15, da esegui.py).

Uso: python sequenza.py [--fino-a FASE] [--solo FASE ...]
Log: <radice>/m16/sequenza.log; uscite di llama-perplexity in <radice>/m16/T-01-<modello>-<passo>.txt.
"""

import argparse
import datetime as dt
import os
import subprocess
import sys
import time
from pathlib import Path

QUI = Path(__file__).resolve().parent
sys.path.insert(0, str(QUI.parent / "batteria"))
import batteria as b  # noqa: E402
import esegui as e  # noqa: E402

PROMPT = QUI.parent / "m08" / "prompt-21k.txt"
CTX = 8192

# Ubatch e batch del profilo, e l'altro ubatch del metro.
MODELLI = {
    "g1": {"pesi": "Qwen_Qwen3.6-35B-A3B-Q4_K_M.gguf", "ub": 4096, "b": 4096, "ub_alt": 512, "b_alt": 4096},
    "g3": {"pesi": "Qwen3-Coder-Next-Q4_K_M.gguf", "ub": 512, "b": 2048, "ub_alt": 2048, "b_alt": 2048},
}
# (passo, build, quale ubatch); il primo passo scrive i logit base.
PASSI = [
    ("base", "moro0", "profilo"),
    ("moro0-ripetuta", "moro0", "profilo"),
    ("moro0-ub-alt", "moro0", "alt"),
    ("moro1", "moro1", "profilo"),
    ("moro1-ub-alt", "moro1", "alt"),
]


def pesi_dir() -> Path:
    import tomllib

    m = tomllib.loads((b.radice() / "machine.toml").read_text(encoding="utf-8"))
    return Path(m["models_dir"])


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--fino-a")
    ap.add_argument("--solo", action="append")
    args = ap.parse_args()
    radice = b.radice()
    out = radice / "m16"
    fasi = out / "fasi"
    fasi.mkdir(parents=True, exist_ok=True)
    diario = open(out / "sequenza.log", "a", encoding="utf-8")

    def scrivi(msg: str) -> None:
        riga = f"[{dt.datetime.now():%Y-%m-%d %H:%M:%S}] {msg}"
        print(riga, flush=True)
        diario.write(riga + "\n")
        diario.flush()

    def attendi() -> None:
        while True:
            o = e.ostacoli(radice, e.RAM_MINIMA_GIB)
            if "llama-perplexity.exe" in e.processi():
                o.append("llama-perplexity.exe è acceso")
            if not o:
                return
            scrivi("aspetto: " + "; ".join(o))
            time.sleep(60)

    def build(serie: str) -> Path:
        return radice / "builds" / f"llama-b10991+{serie}-win-vulkan-x64"

    def kld(sigla: str) -> int:
        cfg = MODELLI[sigla]
        modello = pesi_dir() / cfg["pesi"]
        base = out / f"T-01-{sigla}-base.kld"
        for passo, serie, quale in PASSI:
            res = out / f"T-01-{sigla}-{passo}.txt"
            atteso = "Final estimate" if passo == "base" else "Same top p"
            if res.is_file() and atteso in res.read_text(encoding="utf-8", errors="replace") and base.is_file():
                scrivi(f"{sigla} {passo}: già fatto")
                continue
            ub, bt = (cfg["ub"], cfg["b"]) if quale == "profilo" else (cfg["ub_alt"], cfg["b_alt"])
            cmd = [str(build(serie) / "llama-perplexity.exe"), "-m", str(modello),
                   "-ngl", "999", "-fa", "on", "-c", str(CTX), "-ub", str(ub), "-b", str(bt),
                   "-ctk", "f16", "-ctv", "f16"]
            if passo == "base":
                cmd += ["-f", str(PROMPT), "--kl-divergence-base", str(base)]
            else:
                cmd += ["--kl-divergence-base", str(base), "--kl-divergence"]
            attendi()
            scrivi(f"{sigla} {passo}: {serie} ub {ub} b {bt} · RAM libera {e.ram_libera_gib():.1f} GiB")
            t0 = time.monotonic()
            with open(res, "w", encoding="utf-8") as fh:
                fh.write(f"# {passo} · b10991+{serie} · ub {ub} · b {bt} · {dt.datetime.now().isoformat(timespec='seconds')}\n")
                fh.write("# " + " ".join(cmd) + "\n")
                fh.flush()
                codice = subprocess.run(cmd, stdout=fh, stderr=subprocess.STDOUT).returncode
            scrivi(f"{sigla} {passo}: uscita {codice} in {time.monotonic() - t0:.0f} s")
            if codice != 0:
                return codice
            if passo == "base":
                scrivi(f"{sigla}: file dei logit base {base.stat().st_size / 2**30:.2f} GiB")
        return 0

    def picco() -> int:
        gguf_py = radice / "src" / "llama.cpp" / "gguf-py"
        for sigla in ("g1", "g3"):
            cfg = MODELLI[sigla]
            modello = pesi_dir() / cfg["pesi"]
            for nome, serie, ub, bt in (("moro1", "moro1", cfg["ub"], cfg["b"]),
                                        (f"moro0-ub{cfg['ub_alt']}", "moro0", cfg["ub_alt"], cfg["b_alt"])):
                dump = out / f"T-01-{sigla}-{nome}.kld"
                res = out / f"picco-{sigla}-{nome}.txt"
                fatto = res.is_file() and "Final estimate" in res.read_text(encoding="utf-8", errors="replace")
                if not (fatto and dump.is_file()):
                    cmd = [str(build(serie) / "llama-perplexity.exe"), "-m", str(modello),
                           "-ngl", "999", "-fa", "on", "-c", str(CTX), "-ub", str(ub), "-b", str(bt),
                           "-ctk", "f16", "-ctv", "f16", "-f", str(PROMPT), "--kl-divergence-base", str(dump)]
                    attendi()
                    scrivi(f"picco {sigla} {nome}: salvo i logit · RAM libera {e.ram_libera_gib():.1f} GiB")
                    with open(res, "w", encoding="utf-8") as fh:
                        codice = subprocess.run(cmd, stdout=fh, stderr=subprocess.STDOUT).returncode
                    if codice != 0:
                        scrivi(f"picco {sigla} {nome}: uscita {codice}")
                        return codice
                analisi = out / f"picco-{sigla}-{nome}-per-token.txt"
                env = {**os.environ, "PYTHONPATH": str(gguf_py), "PYTHONIOENCODING": "utf-8"}
                with open(analisi, "w", encoding="utf-8") as fh:
                    codice = subprocess.run(
                        [sys.executable, str(QUI / "kld_per_token.py"), str(out / f"T-01-{sigla}-base.kld"),
                         str(dump), "--gguf", str(modello), "--top", "15"],
                        stdout=fh, stderr=subprocess.STDOUT, env=env).returncode
                scrivi(f"picco {sigla} {nome}: analisi per token, uscita {codice}")
        return 0

    bench = os.environ.get("AETHERA_M08_BENCH") or str(
        QUI.parents[1] / "src-tauri" / "target" / "release" / "examples" / "m08_bench.exe")

    def t02(scenario: str) -> int:
        attendi()
        with open(out / f"{Path(scenario).stem}.out", "a", encoding="utf-8") as fh:
            return subprocess.run([bench, str(radice), str(QUI / scenario), "--riprendi"],
                                  stdout=fh, stderr=subprocess.STDOUT).returncode

    sequenza = [("kld-g1", lambda: kld("g1")), ("kld-g3", lambda: kld("g3")), ("picco", picco),
                ("t02-g1", lambda: t02("T-02a.toml")), ("t02-g3", lambda: t02("T-02b.toml"))]
    esito = 0
    for nome, fn in sequenza:
        if args.solo and nome not in args.solo:
            continue
        segno = fasi / nome
        if segno.exists():
            scrivi(f"fase {nome}: già fatta ({segno.read_text(encoding='utf-8').strip()})")
        else:
            scrivi(f"fase {nome}: inizio · Windows {e.build_windows()}")
            t0 = time.monotonic()
            codice = fn()
            durata = (time.monotonic() - t0) / 60
            if codice == 0:
                segno.write_text(f"{dt.datetime.now():%Y-%m-%d %H:%M} · {durata:.1f} min · Windows {e.build_windows()}\n", encoding="utf-8")
                scrivi(f"fase {nome}: fatta in {durata:.1f} min")
            else:
                scrivi(f"fase {nome}: uscita {codice} dopo {durata:.1f} min")
                esito = 1
        if args.fino_a == nome:
            break
    scrivi("sequenza finita")
    return esito


if __name__ == "__main__":
    sys.exit(main())
