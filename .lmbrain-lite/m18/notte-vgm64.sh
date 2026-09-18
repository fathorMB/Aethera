#!/bin/bash
# M-18, notte del 18/19-09, a VGM 64 dopo il riavvio dell'operatore.
# A 64 GB il Q8_0 del G1 (37,8 GB) sta largo anche a 256k, e il G3 non spilla piu' in condivisa:
# entrambe cose che a VGM 48 non si possono misurare.
#
#   0. controllo che la VGM sia davvero 64 (se e' 48 lo dice e va avanti lo stesso, dichiarandolo)
#   1. Q8_0 del G1 a ctx 262144: ci sta? e quanto costa? (a 48 non entra)
#   2. G3 con gli n-grammi contro il G3 standard, senza spill
#   3. lucidita': G1 a 200k completo (3 posizioni x 2 semi) e G3 a 200k
#   R. prima di tutto il ragionamento: thinking acceso, send_reasoning spento contro acceso, sullo
#      stesso binario del ramo aethera/m18-riuso (target-riuso), su quattro compiti con molti turni
#      (piano in reports/nonio-harness-2026-09.md §7.5)
set -u
. "$(dirname "$0")/../percorsi.sh"
export AETHERA_RADICE AETHERA_REPO AETHERA_PESI PYTHONIOENCODING=utf-8
QUI="$(cd "$(dirname "$0")" && pwd)"; B="$QUI/../batteria"; R=$(cygpath -u "$AETHERA_RADICE")

vgm=$(powershell -NoProfile -Command "[math]::Round((Get-CimInstance Win32_VideoController | Where-Object {\$_.AdapterRAM -gt 0} | Select-Object -First 1).AdapterRAM/1GB)" 2>/dev/null | tr -d '\r')
echo "=== VGM vista dal sistema: ${vgm} GB · $(date +%T)"

bash "$QUI/sentinella.sh" notte64-inizio

echo "=== R. ragionamento: send_reasoning spento contro acceso $(date +%T)"
cd "$B" && python esegui.py --modello G1T --modello G1TR --sessione ragionamento-1 --attendi   --compito rs-lru --compito py-ledger --compito ts-eventi-en --compito py-csvreport
bash "$QUI/sentinella.sh" dopo-ragionamento

echo "=== 1. Q8_0 del G1 a 256k $(date +%T)"
cd "$B" && AETHERA_BATTERIA_CTX=262144 python esegui.py --modello G1Q8 --sessione q8-256k --attendi
bash "$QUI/sentinella.sh" dopo-q8-256k

echo "=== 2. G3 con n-grammi, senza spill $(date +%T)"
cd "$B" && python esegui.py --modello G3N --modello G3 --sessione g3ngram-64 --attendi
bash "$QUI/sentinella.sh" dopo-g3-ngram

echo "=== 3. lucidita' a 200k $(date +%T)"
python "$QUI/lucidita.py" --profilo qwen3.6-35b-a3b.q4_k_m.vulkan --nome g1-200k --profondita 200000 --semi 1
python "$QUI/lucidita.py" --profilo qwen3.6-35b-a3b.q8_0.vulkan --nome q8-200k --profondita 128000 200000 --semi 1
python "$QUI/lucidita.py" --profilo qwen3-coder-next.q4_k_m.vulkan --nome g3-200k --profondita 200000 --semi 1
bash "$QUI/sentinella.sh" dopo-lucidita-200k

echo "M-18 NOTTE VGM64 FINITA $(date +%T)"
