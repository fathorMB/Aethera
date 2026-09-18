#!/bin/bash
# M-17 T-07 e M-18 T-11, in coda alla notte a VGM 64. Tutte con il binario del ramo del riuso.
#   1. thinking acceso, checkpoint spenti: ragionamento rimandato o no (4 compiti). Con i checkpoint
#      accesi (ragionamento-1) non c'era differenza, perche' il server ripartiva dal checkpoint a n-4;
#      senza checkpoint ogni turno con thinking dovrebbe ripartire da zero, se il rimando non c'e'.
#   2. la batteria intera, checkpoint accesi contro spenti, thinking spento: la leva da -82% del
#      costo fisso sul lavoro vero, con l'harness in sola aggiunta.
set -u
. "$(dirname "$0")/../percorsi.sh"
export AETHERA_RADICE AETHERA_REPO AETHERA_PESI PYTHONIOENCODING=utf-8
QUI="$(cd "$(dirname "$0")" && pwd)"; B="$QUI/../batteria"; R=$(cygpath -u "$AETHERA_RADICE")
until grep -q "M-18 NOTTE VGM64 FINITA" "$R/m18/notte-vgm64.out" 2>/dev/null; do sleep 60; done

echo "=== 1. thinking, checkpoint spenti, ragionamento $(date +%T)"
cd "$B" && python esegui.py --modello G1T0 --modello G1TR0 --sessione ragionamento-ckpt0 --attendi \
  --compito rs-lru --compito py-ledger --compito ts-eventi-en --compito py-csvreport
bash "$QUI/sentinella.sh" dopo-ragionamento-ckpt0

echo "=== 2. batteria, checkpoint accesi contro spenti $(date +%T)"
cd "$B" && python esegui.py --modello G1C0 --modello G1B --sessione ckpt0-1 --attendi
bash "$QUI/sentinella.sh" dopo-ckpt0
echo "M-18 NOTTE CKPT0 FINITA $(date +%T)"
