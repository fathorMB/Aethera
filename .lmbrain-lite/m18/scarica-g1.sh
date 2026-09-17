#!/bin/bash
# M-18 T-04: quant alti del G1 (bartowski) con ripresa e SHA-256, poi hard link nella cartella dei
# pesi. Parte solo a motore fermo dopo la coda T-03 di M-17: un download durante una misura la sporca.
set -u
. "$(dirname "$0")/../percorsi.sh"
R=$(cygpath -u "$AETHERA_RADICE"); P=$(cygpath -u "${AETHERA_PESI:?}"); D=$(cygpath -u "${AETHERA_DOWNLOAD:?}")/qwen3.6-35b-a3b
until grep -q "M-17 T-03 FINITO" $R/m17/T-03.out 2>/dev/null; do sleep 30; done
mkdir -p "$D"
base=https://huggingface.co/bartowski/Qwen_Qwen3.6-35B-A3B-GGUF/resolve/main
while read -r name sha; do
  out="$D/$name"
  if [ ! -f "$out" ]; then
    echo "inizio $name $(date +%T)"
    for try in 1 2 3 4 5 6 7 8 9 10; do
      curl -fL --retry 20 --retry-delay 10 --retry-all-errors -C - -o "$out.part" "$base/$name" -sS && break
      echo "tentativo $try fallito per $name"; sleep 15
    done
    got=$(sha256sum "$out.part" | cut -d' ' -f1)
    if [ "$got" = "$sha" ]; then mv "$out.part" "$out"; echo "OK $name $(date +%T)"; else echo "HASH SBAGLIATO $name: $got (il .part resta dov'e')"; continue; fi
  fi
  (cd "$P" && { [ -e "$name" ] || cmd //c mklink //H "$name" "$(cygpath -w "$out")"; })
done <<LIST
Qwen_Qwen3.6-35B-A3B-Q6_K.gguf a97ef79439194d8ec722bc0b6c316f0c41c0b477b35bbae8177c772ecbb0eb91
Qwen_Qwen3.6-35B-A3B-Q8_0.gguf bc57de096ec30e45773aa86103f35a8a32cea01f2193a53a3c636d618e4434e9
LIST
echo "DOWNLOAD G1 FINITO $(date +%T)"
