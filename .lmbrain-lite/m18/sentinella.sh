#!/bin/bash
# M-18: sentinella della deriva della macchina. Il denso 8B con llama-bench (pp512, tg128, 3 giri):
# se tg128 cala oltre il 3% rispetto alla sentinella di inizio notte, il blocco di misure fra le due
# sentinelle va scartato. Uso: sentinella.sh <etichetta>. Una riga JSON in <radice>/m18/sentinella.jsonl.
set -u
. "$(dirname "$0")/../percorsi.sh"
R=$(cygpath -u "$AETHERA_RADICE"); P=$(cygpath -w "${AETHERA_PESI:?}")
B="$R/builds/llama-b10991+moro1-win-vulkan-x64/llama-bench.exe"
while tasklist | grep -qi "llama-server\|m08_bench\|m15_hold\|llama-perplexity\|cargo.exe\|rustc.exe"; do sleep 30; done
mkdir -p $R/m18
"$B" -m "$P\Qwen3-8B-Q4_K_M.gguf" -ngl 999 -fa 1 -p 512 -n 128 -r 3 -o jsonl 2>/dev/null | python -c "
import json,sys,datetime
r={'etichetta':'$1','ora':datetime.datetime.now().isoformat(timespec='seconds')}
for l in sys.stdin:
    l=l.strip()
    if not l.startswith('{'): continue
    d=json.loads(l)
    k='pp512' if d.get('n_prompt') else 'tg128'
    r[k]=round(d['avg_ts'],2); r[k+'_sd']=round(d['stddev_ts'],2)
print(json.dumps(r))" | tee -a $R/m18/sentinella.jsonl
