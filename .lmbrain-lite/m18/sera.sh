#!/bin/bash
# M-18, sera del 18-09, blocchi da meno di un'ora, con sentinella fra l'uno e l'altro:
#   1. Q8_0 secondo giro: il primo dice 13/15 e 35,9 compiti/ora, va confermato;
#   2. G3 con gli n-grammi sulla batteria: stamattina il banco era gia' fatto e lo script l'ha
#      saltato, ma quello che serve e' la batteria (sul G1 ha smentito il banco);
#   3. Q8_0 a ctx 131072: 37,8 GB di pesi piu' la cache, verificare che ci stia in VGM 48.
set -u
. "$(dirname "$0")/../percorsi.sh"
export AETHERA_RADICE AETHERA_REPO AETHERA_PESI PYTHONIOENCODING=utf-8
QUI="$(cd "$(dirname "$0")" && pwd)"; B="$QUI/../batteria"
while tasklist | grep -qi "cargo.exe\|rustc.exe"; do sleep 30; done

bash "$QUI/sentinella.sh" prima-sera
cd "$B" && python esegui.py --modello G1Q8 --sessione quant-2 --attendi
bash "$QUI/sentinella.sh" dopo-q8-giro2

cd "$B" && python esegui.py --modello G3N --modello G3 --sessione g3ngram-1 --attendi
bash "$QUI/sentinella.sh" dopo-g3-ngram

echo "=== Q8_0 a 131k $(date +%T)"
AETHERA_BATTERIA_CTX=131072 python esegui.py --modello G1Q8 --sessione quant-131k --attendi
bash "$QUI/sentinella.sh" dopo-q8-131k
echo "M-18 SERA FINITA $(date +%T)"
