#!/usr/bin/env python3
"""Controlla che la versione di Aethera sia la stessa in tutti i posti in cui è scritta (M-13 T-02).

Posti controllati, a partire dalla radice del repository:
  - src-tauri/tauri.conf.json   campo "version" (è quella che finisce nell'installatore)
  - src-tauri/Cargo.toml        [package] version
  - src-tauri/Cargo.lock        il pacchetto "aethera" (si aggiorna con `cargo check`)
  - package.json                campo "version"
  - titolo della finestra       in tauri.conf.json e in index.html: si controlla solo se
                                contiene un numero di versione (oggi è soltanto «Aethera»)

Con --tag (per esempio v0.1.0) la versione di riferimento è quella del tag senza «v».
Un tag di prova `vX.Y.Z-rc.N` è accettato contro la versione X.Y.Z dei file: la prova di un
workflow non deve costringere a cambiare quattro file e poi a rimetterli a posto.
Senza --tag i file devono solo coincidere fra loro, con tauri.conf.json come riferimento.

Esce con 0 se tutto coincide, con 1 se qualcosa non coincide (e dice quale file correggere),
con 2 se il tag non ha la forma giusta o un file manca. Su GitHub Actions scrive `versione`,
`prova` e `tag` in $GITHUB_OUTPUT.

Uso: python controlla_versione.py [--tag v0.1.0] [--radice CARTELLA]
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import tomllib
from pathlib import Path

TAG = re.compile(r"^v(\d+\.\d+\.\d+)(?:-rc\.(\d+))?$")
VERSIONE = re.compile(r"^\d+\.\d+\.\d+$")
# Un numero di versione dentro un titolo, con o senza «v» davanti.
VERSIONE_NEL_TITOLO = re.compile(r"v?(\d+\.\d+\.\d+(?:-[0-9A-Za-z.]+)?)")


def errore(file: str, messaggio: str) -> None:
    # Il formato ::error:: diventa un'annotazione nella pagina del run.
    if os.environ.get("GITHUB_ACTIONS") == "true":
        print(f"::error file={file}::{messaggio}")
    else:
        print(f"ERRORE {file}: {messaggio}")


def leggi_versioni(radice: Path) -> tuple[list[tuple[str, str | None]], list[tuple[str, str]]]:
    """Restituisce (versioni obbligatorie, titoli con una versione) come coppie (file, valore)."""
    conf_path = radice / "src-tauri" / "tauri.conf.json"
    conf = json.loads(conf_path.read_text(encoding="utf-8-sig"))
    cargo = tomllib.loads((radice / "src-tauri" / "Cargo.toml").read_text(encoding="utf-8-sig"))
    lock = tomllib.loads((radice / "src-tauri" / "Cargo.lock").read_text(encoding="utf-8-sig"))
    pkg = json.loads((radice / "package.json").read_text(encoding="utf-8-sig"))

    nel_lock = next((p.get("version") for p in lock.get("package", []) if p.get("name") == "aethera"), None)
    versioni = [
        ("src-tauri/tauri.conf.json", conf.get("version")),
        ("src-tauri/Cargo.toml", cargo.get("package", {}).get("version")),
        ("src-tauri/Cargo.lock", nel_lock),
        ("package.json", pkg.get("version")),
    ]

    titoli: list[tuple[str, str]] = []
    for finestra in conf.get("app", {}).get("windows", []):
        m = VERSIONE_NEL_TITOLO.search(finestra.get("title", ""))
        if m:
            titoli.append(("src-tauri/tauri.conf.json (titolo della finestra)", m.group(1)))
    index = radice / "index.html"
    if index.is_file():
        t = re.search(r"<title>(.*?)</title>", index.read_text(encoding="utf-8-sig"), re.S)
        if t:
            m = VERSIONE_NEL_TITOLO.search(t.group(1))
            if m:
                titoli.append(("index.html (<title>)", m.group(1)))
    return versioni, titoli


def main() -> int:
    # Accenti e «virgolette» leggibili anche su una console Windows in cp1252.
    sys.stdout.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--tag", help="tag della release, per esempio v0.1.0 o v0.1.0-rc.1")
    ap.add_argument("--radice", default=".", help="radice del repository (predefinita: cartella corrente)")
    args = ap.parse_args()
    radice = Path(args.radice)

    prova = False
    riferimento_da = "src-tauri/tauri.conf.json"
    if args.tag is not None:
        m = TAG.match(args.tag)
        if not m:
            errore(
                args.tag,
                f"il tag «{args.tag}» non ha la forma vX.Y.Z né vX.Y.Z-rc.N: cancellalo e rimettilo "
                "con la forma giusta",
            )
            return 2
        riferimento, prova = m.group(1), m.group(2) is not None
        riferimento_da = f"tag {args.tag}"

    try:
        versioni, titoli = leggi_versioni(radice)
    except (OSError, ValueError) as e:
        errore(str(radice), f"non riesco a leggere i file della versione: {e}")
        return 2

    if args.tag is None:
        riferimento = versioni[0][1] or ""
        if not VERSIONE.match(riferimento):
            errore("src-tauri/tauri.conf.json", f"«version» vale «{riferimento}», serve X.Y.Z")
            return 1

    sbagliati = 0
    for file, valore in versioni + titoli:
        if valore == riferimento:
            print(f"ok      {file}: {valore}")
            continue
        sbagliati += 1
        if valore is None:
            errore(file, f"la versione manca; scrivi {riferimento} ({riferimento_da})")
        elif file == "src-tauri/Cargo.lock":
            errore(file, f"dice {valore}, il riferimento ({riferimento_da}) è {riferimento}: "
                         "correggi prima Cargo.toml, poi lancia `cargo check` in src-tauri e committa il lock")
        else:
            errore(file, f"dice {valore}, il riferimento ({riferimento_da}) è {riferimento}: correggi questo file")

    if sbagliati:
        print(f"\nPosti da correggere: {sbagliati}. La versione va scritta uguale in tutti; "
              "vedi «Come si fa una release» nel README.")
        return 1

    tipo = "di prova (rc)" if prova else "definitiva" if args.tag else "senza tag"
    print(f"\nVersione {riferimento} coerente ovunque; release {tipo}.")
    uscita = os.environ.get("GITHUB_OUTPUT")
    if uscita:
        with open(uscita, "a", encoding="utf-8") as f:
            f.write(f"versione={riferimento}\n")
            f.write(f"prova={'true' if prova else 'false'}\n")
            f.write(f"tag={args.tag or ''}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
