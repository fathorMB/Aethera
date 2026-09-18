#!/bin/bash
# M-17 T-05: le leve, una alla volta, sullo scenario di T-01. T-03 ha detto che il costo fisso è
# per il 94% la copia dei checkpoint dello stato ricorrente (3 per richiesta sul G3, 226 MiB).
# Quindi la leva che conta è --ctx-checkpoints: a 0 nessun checkpoint, ma una divergenza dentro il
# generato costa il ricalcolo completo. Per un harness che scrive solo in coda la divergenza non
# capita mai: la misura dice quanto vale, e «divergenza» misura quanto costa quando capita.
set -u
. "$(dirname "$0")/../percorsi.sh"
S="$(cygpath -w "$(dirname "$0")/scenario.py")"
G3=qwen3-coder-next.q4_k_m.vulkan; G1=qwen3.6-35b-a3b.q4_k_m.vulkan
for m in "g3:$G3:" "g1:$G1:--senza-thinking"; do
  sig=${m%%:*}; resto=${m#*:}; prof=${resto%%:*}; extra=${resto#*:}
  python "$S" --misura T-05 --profilo $prof --nome $sig-ckpt0  --ctx-checkpoints 0 $extra
  python "$S" --misura T-05 --profilo $prof --nome $sig-ckpt1  --ctx-checkpoints 1 $extra
  python "$S" --misura T-05 --profilo $prof --nome $sig-cacheram0 --cache-ram 0 $extra
done
# Che cosa costa una divergenza quando i checkpoint non ci sono: stesso scenario ma con la coda
# riscritta a ogni turno (--divergenza), su G3, con e senza checkpoint.
python "$S" --misura T-05 --profilo $G3 --nome g3-div-ckpt32 --divergenza
python "$S" --misura T-05 --profilo $G3 --nome g3-div-ckpt0  --divergenza --ctx-checkpoints 0
echo "M-17 T-05 FINITO $(date +%T)"
