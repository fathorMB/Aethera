#!/bin/bash
# M-17 T-01 e T-02: il costo fisso sui profili standard (G3, G1), poi i metri: il denso 8B, non ibrido,
# sulla stessa conversazione, e /completion al posto di /v1/chat/completions.
# scenario.py aspetta da solo che il motore sia libero.
set -u
. "$(dirname "$0")/../percorsi.sh"
S="$(cygpath -w "$(dirname "$0")/scenario.py")"
G3=qwen3-coder-next.q4_k_m.vulkan; G1=qwen3.6-35b-a3b.q4_k_m.vulkan; D8=qwen3-8b.q4_k_m.vulkan.m17
python "$S" --misura T-01 --profilo $G3 --nome g3-standard
python "$S" --misura T-01 --profilo $G1 --nome g1-standard --senza-thinking
python "$S" --misura T-02 --profilo $D8 --nome d8-chat --senza-thinking
python "$S" --misura T-02 --profilo $G3 --nome g3-completion --endpoint completion
python "$S" --misura T-02 --profilo $G1 --nome g1-completion --endpoint completion
echo "M-17 T-01/T-02 FINITO $(date +%T)"
