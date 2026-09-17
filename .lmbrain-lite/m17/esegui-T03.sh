#!/bin/bash
# M-17 T-03: lo scenario di T-01 con il log del motore a verbosità 5 (un giro), per cronometrare le
# fasi dentro prompt_ms dalle righe «new prompt», «cached n_tokens», «created context checkpoint»,
# «n_batch (effective)». Il log resta in <radice>/runs/<avvio>/server.log (avvio nel .pronto.json).
set -u
. "$(dirname "$0")/../percorsi.sh"
S="$(cygpath -w "$(dirname "$0")/scenario.py")"
G3=qwen3-coder-next.q4_k_m.vulkan; G1=qwen3.6-35b-a3b.q4_k_m.vulkan; D8=qwen3-8b.q4_k_m.vulkan.m17
python "$S" --misura T-03 --profilo $G3 --nome g3-lv5 --giri 1 --extra "-lv 5"
python "$S" --misura T-03 --profilo $G1 --nome g1-lv5 --giri 1 --extra "-lv 5" --senza-thinking
python "$S" --misura T-03 --profilo $D8 --nome d8-lv5 --giri 1 --extra "-lv 5" --senza-thinking
echo "M-17 T-03 FINITO $(date +%T)"
