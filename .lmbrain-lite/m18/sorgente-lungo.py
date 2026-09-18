"""M-18: un sorgente lungo e congelato per le conversazioni in profondità (64k-256k token).

Concatena i .cpp del server e del nucleo di llama.cpp (il fork in <radice>/src/llama.cpp, tag
b10991), in ordine alfabetico, fino a --caratteri. Scrive <radice>/m18/sorgente-lungo.txt e il suo
SHA-256 accanto: il file non sta nel repository, si rigenera uguale dallo stesso tag.
"""
import argparse
import hashlib
import os
import sys
from pathlib import Path

ap = argparse.ArgumentParser()
ap.add_argument("--caratteri", type=int, default=1_000_000)
args = ap.parse_args()
radice = Path(os.environ.get("AETHERA_RADICE") or sys.exit("AETHERA_RADICE non impostata"))
src = radice / "src" / "llama.cpp"
file = sorted((src / "tools" / "server").glob("*.cpp")) + sorted((src / "src").glob("*.cpp"))
pezzi, tot = [], 0
for f in file:
    t = f"// ===== {f.relative_to(src).as_posix()} =====\n" + f.read_text(encoding="utf-8", errors="replace")
    pezzi.append(t)
    tot += len(t)
    if tot >= args.caratteri:
        break
testo = "".join(pezzi)[: args.caratteri]
out = radice / "m18"
out.mkdir(parents=True, exist_ok=True)
(out / "sorgente-lungo.txt").write_text(testo, encoding="utf-8", newline="\n")
sha = hashlib.sha256(testo.encode("utf-8")).hexdigest()
(out / "sorgente-lungo.sha256").write_text(sha + "\n", encoding="utf-8")
print(f"{len(testo)} caratteri · {len(pezzi)} file · sha256 {sha}")
