#!/usr/bin/env python3
"""Estrae da RELEASE-NOTES.md il corpo della GitHub Release (M-13 T-01).

Il corpo è fatto di:
  1. la sezione della versione: il titolo «## X.Y.Z …» (con o senza «v») e tutto quello che segue
     fino al titolo «## » successivo;
  2. la sezione «## Installatore non firmato …», se c'è: vale per ogni versione e sta in fondo
     al file, quindi va aggiunta a mano;
  3. per un tag di prova, una riga in testa che lo dice.

Se il file non ha una sezione per quella versione, il corpo è tutto il file senza il titolo «# »
iniziale, e lo script lo segnala con un avviso: meglio note in più che una release senza note.

Uso: python note_di_rilascio.py --versione 0.1.0 [--tag v0.1.0-rc.1] [--file RELEASE-NOTES.md] --uscita corpo.md
"""

from __future__ import annotations

import argparse
import os
import re
import sys
from pathlib import Path

NON_FIRMATO = re.compile(r"^##\s+Installatore non firmato", re.I)


def avviso(messaggio: str) -> None:
    if os.environ.get("GITHUB_ACTIONS") == "true":
        print(f"::warning file=RELEASE-NOTES.md::{messaggio}")
    else:
        print(f"AVVISO: {messaggio}")


def sezioni(testo: str) -> list[tuple[str, str]]:
    """Divide il testo nelle sezioni di secondo livello: (titolo, testo completo col titolo)."""
    out: list[tuple[str, list[str]]] = []
    for riga in testo.splitlines():
        if riga.startswith("## "):
            out.append((riga, [riga]))
        elif out:
            out[-1][1].append(riga)
    return [(t, "\n".join(r).strip()) for t, r in out]


def main() -> int:
    # Accenti e «virgolette» leggibili anche su una console Windows in cp1252.
    sys.stdout.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--versione", required=True)
    ap.add_argument("--tag", default="")
    ap.add_argument("--file", default="RELEASE-NOTES.md")
    ap.add_argument("--uscita", required=True)
    args = ap.parse_args()

    testo = Path(args.file).read_text(encoding="utf-8-sig")
    tutte = sezioni(testo)
    titolo_versione = re.compile(rf"^##\s+v?{re.escape(args.versione)}(?![0-9A-Za-z.\-])")
    della_versione = next((s for t, s in tutte if titolo_versione.match(t)), None)
    non_firmato = next((s for t, s in tutte if NON_FIRMATO.match(t)), None)

    parti: list[str] = []
    if args.tag and "-rc." in args.tag:
        parti.append(f"> **Versione di prova** ({args.tag}): serve a provare il workflow e "
                     f"l'installatore, non è la {args.versione}.")
    if della_versione:
        parti.append(della_versione)
        if non_firmato:
            parti.append(non_firmato)
    else:
        avviso(f"nessuna sezione «## {args.versione}» in {args.file}: uso tutto il file come note")
        parti.append(re.sub(r"\A#\s[^\n]*\n", "", testo.lstrip("﻿")).strip())

    Path(args.uscita).write_text("\n\n".join(parti) + "\n", encoding="utf-8")
    print(f"Note scritte in {args.uscita}: {sum(len(p) for p in parti)} caratteri.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
