#!/usr/bin/env bash
# M-08 T-03 — l'SSD sul file GGUF vero, non su un file di prova.
#
# DiskSpd in sola lettura, unbuffered (-Sh: niente cache del sistema), sul GGUF del 35B: 4 KiB e
# 2 MiB, casuale (-r) e sequenziale (-s), code 1/8/32, un thread. Venti secondi per combinazione.
#
# Serve a scegliere la colonna giusta quando si ragiona di streaming dei pesi da disco: la banda
# sequenziale dichiarata dal produttore non è quella che vede chi legge blocchi sparsi.

set -u
DISKSPD="${DISKSPD:?percorso di diskspd.exe}"
. "$(dirname "$0")/../percorsi.sh"
FILE="${FILE:-${AETHERA_PESI:?imposta AETHERA_PESI}/Qwen_Qwen3.6-35B-A3B-Q4_K_M.gguf}"
OUT="${1:-$AETHERA_RADICE/m08}"
mkdir -p "$OUT"
RES="$OUT/T-03-diskspd.txt"
: > "$RES"

run() {
  local label="$1"; shift
  echo "--- $label ---" | tee -a "$RES"
  echo "diskspd $* $FILE" >> "$RES"
  "$DISKSPD" "$@" "$FILE" >> "$RES" 2>&1
  grep -E "^total:" -A 3 "$RES" | tail -3
}

for qd in 1 8 32; do
  run "4 KiB casuale QD$qd"      -b4K  -r -o"$qd" -t1 -d20 -Sh -w0 -L
  run "2 MiB casuale QD$qd"      -b2M  -r -o"$qd" -t1 -d20 -Sh -w0 -L
done
run "2 MiB sequenziale QD8"      -b2M  -s -o8  -t1 -d20 -Sh -w0 -L
run "2 MiB sequenziale QD32"     -b2M  -s -o32 -t1 -d20 -Sh -w0 -L
run "1 MiB sequenziale QD32 4 thread" -b1M -s -o32 -t4 -d20 -Sh -w0 -L

echo "risultati in $RES"
