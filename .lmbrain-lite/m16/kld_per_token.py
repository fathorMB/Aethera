"""M-16 T-01 — KLD token per token fra due file di logit di llama-perplexity.

`llama-perplexity` stampa la KLD solo per blocco e in sintesi. Qui si confrontano due file scritti
con `--kl-divergence-base` (la base e, come «base» a sua volta, la variante), token per token.

    python kld_per_token.py <base.kld> <variante.kld> [--gguf modello.gguf] [--top 15] [--soglia 0.5]

Formato del file (tools/perplexity/perplexity.cpp): "_logits_", n_ctx (u32), n_vocab (i32),
n_chunk (i32), i token (i32 × n_ctx × n_chunk), poi per ogni blocco n_ctx-1-n_ctx/2 record da
nv = 2*((n_vocab+1)/2)+4 uint16. I primi 4 uint16 di un record sono due float32 (scala e log-prob
minimo); gli altri sono le log-probabilità quantizzate a 16 bit sull'intervallo [max-16, max].
Il record i del blocco c predice il token in posizione c*n_ctx + n_ctx/2 + i + 1.

La KLD si calcola come llama-perplexity: somma su p_base dove log p_base > -16. Anche la variante
qui è quantizzata a 16 bit: il pavimento del confronto è ~1e-4 (lo si vede con base contro base).

Stampa: distribuzione, i token peggiori con il contesto, i gruppi (token sopra soglia a meno di 32
posizioni l'uno dall'altro) e, con --gguf, i tipi dei tensori del modello.
"""
import argparse
import sys
from pathlib import Path

import numpy as np


def leggi(path):
    raw = np.memmap(path, dtype=np.uint8, mode="r")
    assert bytes(raw[:8]) == b"_logits_", f"{path}: non è un file di logit"
    n_ctx = int(np.frombuffer(raw[8:12], np.uint32)[0])
    n_vocab, n_chunk = (int(x) for x in np.frombuffer(raw[12:20], np.int32))
    off = 20
    tokens = np.frombuffer(raw[off:off + 4 * n_ctx * n_chunk], np.int32)
    off += 4 * n_ctx * n_chunk
    nv = 2 * ((n_vocab + 1) // 2) + 4
    per_blocco = n_ctx - 1 - n_ctx // 2
    rec = np.frombuffer(raw[off:off + 2 * nv * per_blocco * n_chunk], np.uint16).reshape(n_chunk * per_blocco, nv)
    return {"n_ctx": n_ctx, "n_vocab": n_vocab, "n_chunk": n_chunk, "tokens": tokens, "rec": rec, "per_blocco": per_blocco}


def logp(riga, n_vocab):
    scala, minimo = np.frombuffer(riga[:4].tobytes(), np.float32)
    q = riga[4:4 + n_vocab].astype(np.float32)
    return scala * q + minimo


def vocabolario(gguf):
    try:
        from gguf import GGUFReader  # gguf-py di llama.cpp (PYTHONPATH)
    except ImportError:
        return None, None
    r = GGUFReader(gguf)
    f = r.fields["tokenizer.ggml.tokens"]
    voc = [bytes(f.parts[i]).decode("utf-8", "replace") for i in f.data]
    tipi = {}
    for t in r.tensors:
        parti = t.name.split(".")
        parti = parti[2:] if parti[0] == "blk" else parti
        nome = ".".join(x for x in parti if x not in ("weight", "bias")) or t.name
        tipi.setdefault(t.tensor_type.name, set()).add(nome)
    return voc, tipi


def testo(voc, ids):
    if voc is None:
        return " ".join(str(i) for i in ids)
    # byte-level BPE: Ġ è lo spazio, Ċ l'a capo
    return "".join(voc[i] for i in ids).replace("Ġ", " ").replace("Ċ", "⏎").replace("ĉ", "⇥")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("base")
    ap.add_argument("variante")
    ap.add_argument("--gguf")
    ap.add_argument("--top", type=int, default=15)
    ap.add_argument("--soglia", type=float, default=0.5)
    a = ap.parse_args()
    b, v = leggi(a.base), leggi(a.variante)
    assert (b["n_ctx"], b["n_vocab"], b["n_chunk"]) == (v["n_ctx"], v["n_vocab"], v["n_chunk"]), "file non confrontabili"
    assert np.array_equal(b["tokens"], v["tokens"]), "token diversi"
    n_vocab, n_ctx, pb = b["n_vocab"], b["n_ctx"], b["per_blocco"]
    voc, tipi = vocabolario(a.gguf) if a.gguf else (None, None)
    n = len(b["rec"])
    kld = np.empty(n)
    righe = []
    for i in range(n):
        lb, lv = logp(b["rec"][i], n_vocab), logp(v["rec"][i], n_vocab)
        m = lb > -16  # come llama-perplexity
        pb_ = np.exp(lb[m])
        kld[i] = float(np.sum(pb_ * (lb[m] - lv[m])))
        c, j = divmod(i, pb)
        pos = c * n_ctx + n_ctx // 2 + j
        vero = int(b["tokens"][pos + 1])
        tb, tv = int(lb.argmax()), int(lv.argmax())
        righe.append((pos, vero, tb, tv, float(np.exp(lb[tb])), float(np.exp(lv[tb])), float(np.exp(lv[tv])),
                      float(np.exp(lb[tv])), float(np.exp(lb[vero])), float(np.exp(lv[vero]))))
    q = np.quantile(kld, [0.5, 0.9, 0.99, 0.999])
    print(f"token {n} · KLD media {kld.mean():.6f} · mediana {q[0]:.6f} · 90% {q[1]:.6f} · 99% {q[2]:.6f} · 99,9% {q[3]:.6f} · max {kld.max():.4f}")
    stesso = sum(r[2] == r[3] for r in righe)
    print(f"stesso token più probabile: {stesso}/{n} ({100 * stesso / n:.2f}%)")
    for s in (0.1, 0.5, 1.0, 2.0, 5.0):
        print(f"  token con KLD > {s}: {int((kld > s).sum())}")
    print(f"\nI {a.top} token peggiori (posizione nel testo del prompt da 21k):")
    for i in np.argsort(-kld)[:a.top]:
        pos, vero, tb, tv, pbb, pvb, pvv, pbv, pbt, pvt = righe[i]
        ctx = testo(voc, b["tokens"][max(0, pos - 15):pos + 1].tolist())
        print(f"- KLD {kld[i]:.3f} · pos {pos} (blocco {pos // n_ctx}) · contesto «…{ctx}»")
        print(f"    base: primo «{testo(voc, [tb])}» p={pbb:.3f} (variante gli dà {pvb:.3f}) · variante: primo «{testo(voc, [tv])}» p={pvv:.3f} (base gli dà {pbv:.3f})")
        print(f"    token vero «{testo(voc, [vero])}»: p base {pbt:.3f} · p variante {pvt:.3f}")
    alti = sorted(i for i in range(n) if kld[i] > a.soglia)
    gruppi, g = [], []
    for i in alti:
        if g and righe[i][0] - righe[g[-1]][0] > 32:
            gruppi.append(g)
            g = []
        g.append(i)
    if g:
        gruppi.append(g)
    print(f"\nGruppi di token con KLD > {a.soglia} (distanza ≤ 32): {len(gruppi)}")
    for g in gruppi:
        print(f"  pos {righe[g[0]][0]}–{righe[g[-1]][0]}: {len(g)} token, KLD max {max(kld[i] for i in g):.3f}")
    if tipi:
        print("\nTipi dei tensori del modello:")
        for t, nomi in sorted(tipi.items()):
            print(f"  {t}: {', '.join(sorted(nomi))}")


if __name__ == "__main__":
    main()
