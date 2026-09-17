"""M-15 — congela la batteria: per ogni compito controlla che il verificatore fallisca sul fixture
com'è e passi con la soluzione di riferimento, poi scrive gli hash in congelato.json.

Uso:  python congela.py [--solo-controllo] [--lavoro CARTELLA]
  --solo-controllo  non riscrive congelato.json: verifica soltanto e confronta gli hash
  --lavoro          dove creare le copie di prova (di default una cartella temporanea)

Controlli per compito:
  1. fixture intatto               -> il verificatore deve FALLIRE
  2. fixture + soluzione           -> deve PASSARE
  3. soluzione + un file protetto  -> deve FALLIRE (per i compiti con file protetti oltre ai test)
  4. soluzione + un test visibile svuotato -> deve FALLIRE (i test non si toccano)
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import shutil
import sys
import tempfile
from pathlib import Path

import batteria as b


def prova(compito: dict, base: Path, nome: str, dopo=None) -> dict:
    dest = base / f"{compito['id']}-{nome}"
    if dest.exists():
        shutil.rmtree(dest)
    b.prepara(compito, dest)
    if dopo:
        dopo(dest)
    return b.verifica(compito, dest)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--solo-controllo", action="store_true")
    ap.add_argument("--lavoro")
    ap.add_argument("--compito", action="append", help="solo questi id")
    args = ap.parse_args()

    compiti = [c for c in b.carica() if not args.compito or c["id"] in args.compito]
    base = Path(args.lavoro) if args.lavoro else Path(tempfile.mkdtemp(prefix="m15-congela-"))
    base.mkdir(parents=True, exist_ok=True)
    ids = [c["id"] for c in compiti]
    if len(set(ids)) != len(ids):
        print("id duplicati", file=sys.stderr)
        return 2

    errori = []
    risultati = {}
    for c in compiti:
        righe = []
        r1 = prova(c, base, "fixture")
        righe.append(("fixture fallisce", not r1["passato"]))
        r2 = prova(c, base, "soluzione", lambda d, c=c: b.applica_soluzione(c, d))
        righe.append(("soluzione passa", r2["passato"]))
        extra = [p for p in c["protetti"] if not p.startswith("tests/") and p not in ("Cargo.toml", "package.json")]
        if extra:
            bersaglio = next(
                (r for r in b.file_di(b.FIXTURE / c["fixture"]) if any(b._combacia(r.as_posix(), s) for s in extra)),
                None,
            )

            def tocca(d, c=c, rel=bersaglio):
                b.applica_soluzione(c, d)
                with open(d / rel, "a", encoding="utf-8") as fh:
                    fh.write("\n# ritocco\n")

            r3 = prova(c, base, "protetto", tocca)
            righe.append((f"toccare {bersaglio.as_posix()} fallisce", not r3["passato"]))

        def svuota_test(d, c=c):
            b.applica_soluzione(c, d)
            fx = b.FIXTURE / c["fixture"]
            test = next(r for r in b.file_di(fx) if r.parts[0] == "tests" and (fx / r).stat().st_size > 0)
            (d / test).write_text("", encoding="utf-8")

        r4 = prova(c, base, "test-svuotato", svuota_test)
        righe.append(("svuotare un test fallisce", not r4["passato"]))

        ok = all(v for _, v in righe)
        stato = "ok " if ok else "NO "
        print(f"{stato} {c['id']:<14} " + " · ".join(f"{k}: {'sì' if v else 'NO'}" for k, v in righe))
        if not r2["passato"]:
            for cmd in r2["comandi"]:
                print("   ", cmd["comando"], cmd["codice"], cmd["coda"][-600:].replace("\n", "\n    "))
            print("   ", r2["protetti_violati"], r2["controlli"])
        if not ok:
            errori.append(c["id"])
        risultati[c["id"]] = {
            "livello": c["livello"],
            "lingua": c["lingua"],
            "tipo": c["tipo"],
            "toolchain": c["toolchain"],
            "max_turni": c["max_turni"],
            "max_secondi": c["max_secondi"],
            "impronte": b.impronte(c),
            "prova": {k: v for k, v in righe},
            "secondi_verifica_soluzione": round(sum(x["secondi"] for x in r2["comandi"]), 2),
        }

    if not args.lavoro:
        shutil.rmtree(base, ignore_errors=True)

    if args.solo_controllo:
        diff = b.controlla_congelato(compiti)
        print("hash: " + ("coincidono con congelato.json" if not diff else "; ".join(diff)))
        return 1 if (errori or diff) else 0

    if errori:
        print(f"non congelo: {len(errori)} compiti non superano le prove: {errori}", file=sys.stderr)
        return 1
    doc = {
        "versione": 1,
        "congelato_il": dt.date.today().isoformat(),
        "nota": "scritto da congela.py; ogni compito è stato provato: fixture che fallisce, soluzione che passa",
        "compiti": risultati,
    }
    b.CONGELATO.write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
    print(f"congelati {len(risultati)} compiti in {b.CONGELATO.name}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
