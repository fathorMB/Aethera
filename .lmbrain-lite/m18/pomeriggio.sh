#!/bin/bash
# M-18, pomeriggio del 18-09: quello che sta in un'ora per blocco, dopo la lucidita' del G3.
#   1. speculativa a n-grammi sul G3, dove il confronto e' contro «niente» e non contro MTP (T-12a
#      l'ha misurata sul banco: qui la misura la batteria, che ieri ha smentito il banco sul G1);
#   2. scala dei quant del G1: Q6_K e Q8_0 contro il Q4_K_M standard (T-04).
# Sentinella fra un blocco e l'altro; i profili standard non si toccano.
set -u
. "$(dirname "$0")/../percorsi.sh"
export AETHERA_RADICE AETHERA_REPO AETHERA_PESI
export PYTHONIOENCODING=utf-8
export AETHERA_NONIO_EXE="C:\Git\Nonio\target\release\nonio.exe"
QUI="$(cd "$(dirname "$0")" && pwd)"
B="$QUI/../batteria"
until grep -q "LUCIDITA G3 FINITA" "$(cygpath -u "$AETHERA_RADICE")/m18/lucidita-g3.out" 2>/dev/null; do sleep 60; done

bash "$QUI/sentinella.sh" prima-pomeriggio
echo "=== n-grammi sul G3, banco $(date +%T)"
"$(cygpath -u "$AETHERA_REPO")/src-tauri/target/release/examples/m08_bench.exe" \
  "$(cygpath -w "$AETHERA_RADICE")" "$(cygpath -w "$QUI/T-12a-spec-g3.toml")" --riprendi 2>&1 | tail -20
bash "$QUI/sentinella.sh" dopo-spec-g3-banco

echo "=== quant del G1 sulla batteria $(date +%T)"
cd "$B" && python esegui.py --modello G1Q8 --modello G1Q6 --sessione quant-1 --attendi
bash "$QUI/sentinella.sh" dopo-quant
echo "M-18 POMERIGGIO FINITO $(date +%T)"
