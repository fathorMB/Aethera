"""M-08 T-11 — la NPU da sola, col 35B spento: il timeout del motore (LiveKernelEvent 141) viene dal
driver NPU o dalla contesa con la iGPU? Stessi carichi del driver T-11-npu.py, riga per riga su
disco, così un crash non si porta via niente.

Uso: python T-11-npu-sola.py [minuti-embedding] [minuti-chat]
"""
import importlib.util
import json
import sys
import threading
import time
from pathlib import Path

spec = importlib.util.spec_from_file_location("t11", Path(__file__).with_name("T-11-npu.py"))
t11 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(t11)

OUT = Path("C:/AetheraData/m08/T-11-npu-sola.jsonl")


class Rows(list):
    """Scrive ogni riga appena arriva."""

    def __init__(self, fase):
        super().__init__()
        self.fase = fase

    def append(self, r):
        super().append(r)
        with open(OUT, "a", encoding="utf-8") as f:
            f.write(json.dumps(dict(r, condition="npu-sola", fase=self.fase)) + "\n")
        if len(self) % 20 == 0 or "error" in r:
            print(self.fase, len(self), r.get("error") or r.get("wall_ms"), flush=True)


for fase, minuti in (("embedding", float(sys.argv[1]) if len(sys.argv) > 1 else 10),
                     ("chat", float(sys.argv[2]) if len(sys.argv) > 2 else 5)):
    stop, rows = threading.Event(), Rows(fase)
    th = threading.Thread(target=t11.npu_loop, args=(fase if fase == "embedding" else "genera", stop, rows), daemon=True)
    th.start()
    time.sleep(minuti * 60)
    stop.set()
    th.join(timeout=300)
    print(f"{fase}: {len(rows)} richieste, errori {sum('error' in r for r in rows)}", flush=True)
