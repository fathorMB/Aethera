#!/usr/bin/env bash
# M-08 T-11 — la NPU come secondo motore, mentre il primo lavora.
#
# La domanda non e' «quanto va la NPU»: e' **quanto rallenta il motore principale** quando la NPU
# lavora insieme a lui. Su questa macchina la memoria e' una sola (UMA): iGPU e NPU pescano dalla
# stessa LPDDR5X, e T-02 ha appena mostrato che il decode del 35B e' esattamente un problema di
# banda. Se la contesa esiste, si vede li'.
#
# Tre misure, nell'ordine:
#   A  35B da solo               (riferimento, lo stesso carico di T-05)
#   B  35B mentre la NPU tiene acceso EmbeddingGemma e lo interroga in continuo
#   C  35B mentre la NPU genera con Qwen3.5-4B
#
# FastFlowLM si usa nella versione portatile (zip scompattato), senza installare niente: la CLI e'
# MIT, i kernel NPU sono binari proprietari liberi per uso non commerciale o sotto i 10 M$ di
# fatturato (TERMS.md del progetto).
#
# Uso: FLM=<cartella con flm.exe> bash run-T11-npu.sh

set -u
FLM="${FLM:?cartella di flm.exe}"
HERE="$(cd "$(dirname "$0")" && pwd)"
OUT="${OUT:-C:/AetheraData/m08}"
mkdir -p "$OUT"

echo "=== stack NPU ==="
"$FLM/flm.exe" validate --json 2>&1 | tee "$OUT/T-11-validate.json" | head -20
"$FLM/flm.exe" version 2>&1 | head -3
