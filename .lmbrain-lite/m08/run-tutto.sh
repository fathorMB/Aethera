#!/usr/bin/env bash
# M-08 — la notte intera, in ordine. Una misura per volta: il motore e' uno solo e le misure non
# si sovrappongono, altrimenti misurano la contesa e non la leva.
#
# Uso: bash run-tutto.sh <radice> <cartella-esempi> [misure…]
#   bash run-tutto.sh <radice> <repo>/src-tauri/target/release/examples T-04 T-05

set -u
. "$(dirname "$0")/../percorsi.sh"
ROOT="${1:-$AETHERA_RADICE}"
EX="${2:-$AETHERA_REPO/src-tauri/target/release/examples}"
shift 2 2>/dev/null || true
HERE="$(cd "$(dirname "$0")" && pwd)"
MISURE=("$@")
[ ${#MISURE[@]} -eq 0 ] && MISURE=(T-04 T-05 T-06 T-07 T-08 T-09)

file_di() {
  case "$1" in
    T-00) echo "T-00-prova.toml" ;;
    T-04) echo "T-04-checkpoint.toml" ;;
    T-05) echo "T-05-mtp.toml" ;;
    T-06) echo "T-06-kvq8.toml" ;;
    T-07) echo "T-07-build.toml" ;;
    T-08) echo "T-08-oltre48.toml" ;;
    T-09) echo "T-09-loadmode.toml" ;;
    T-13) echo "T-13-ubatch.toml" ;;
    *) echo "" ;;
  esac
}

for m in "${MISURE[@]}"; do
  f="$(file_di "$m")"
  if [ -z "$f" ]; then
    echo "### $m: nessuno scenario, saltata"
    continue
  fi
  echo ""
  echo "#################### $m — $(date '+%H:%M:%S') ####################"
  # Il motore deve essere spento e la porta libera prima di ogni misura.
  if [ -n "$(tasklist //FI "IMAGENAME eq llama-server.exe" //NH 2>/dev/null | grep -i llama-server || true)" ]; then
    echo "ATTENZIONE: c'e' gia' un llama-server acceso. $m saltata per non misurare la contesa."
    continue
  fi
  "$EX/m08_bench.exe" "$ROOT" "$HERE/$f" || echo "### $m: uscita con errore, si prosegue con la prossima"
  sleep 10
done

echo ""
echo "#################### finito — $(date '+%H:%M:%S') ####################"
