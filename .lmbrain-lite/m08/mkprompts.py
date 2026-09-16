"""M-08: congela i due prompt di codice vero del banco.

Non una frase ripetuta: sorgenti veri di questo repository, concatenati nell'ordine dichiarato e
tagliati su un confine di riga. Il numero di token si misura con `/tokenize` del motore acceso, non
si stima: si parte da una lunghezza in caratteri e si aggiusta finché il conto non cade nella
tolleranza.

Uso: python mkprompts.py [http://127.0.0.1:8080]
Senza motore acceso scrive comunque i file con la stima a 3,4 caratteri per token e lo dichiara.
"""
import json
import os
import sys
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.abspath(os.path.join(HERE, "..", "..", "src-tauri", "src"))

# Ordine congelato: cambiarlo cambierebbe il prompt, e quindi tutte le misure.
FILES_7K = ["cmdline.rs", "estimate.rs", "telemetry.rs", "memory.rs", "diagnose.rs"]
FILES_21K = FILES_7K + ["catalog.rs", "download.rs", "profile.rs", "engine.rs", "gguf.rs", "launch.rs", "machine.rs"]

TARGETS = [("prompt-7k.txt", FILES_7K, 7000), ("prompt-21k.txt", FILES_21K, 21000)]
TOLERANCE = 0.02


def corpus(files):
    parts = []
    for name in files:
        with open(os.path.join(SRC, name), encoding="utf-8") as fh:
            parts.append("// ==== src/%s ====\n%s" % (name, fh.read()))
    return "\n".join(parts)


def cut(text, chars):
    if chars >= len(text):
        return text
    end = text.rfind("\n", 0, chars)
    return text[: end if end > 0 else chars]


def count_tokens(url, text):
    body = json.dumps({"content": text}).encode("utf-8")
    req = urllib.request.Request(url + "/tokenize", data=body, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=300) as r:
        return len(json.load(r)["tokens"])


def main():
    url = sys.argv[1] if len(sys.argv) > 1 else None
    report = {}
    for name, files, target in TARGETS:
        text = corpus(files)
        if url is None:
            chars = int(target * 3.4)
            out = cut(text, chars)
            report[name] = {"tokens": None, "chars": len(out), "note": "stimato a 3,4 caratteri/token: motore spento"}
        else:
            lo, hi = 1000, len(text)
            out, tokens = text, count_tokens(url, text)
            if tokens < target:
                raise SystemExit("%s: il corpus intero fa %d token, sotto i %d richiesti" % (name, tokens, target))
            for _ in range(20):
                mid = (lo + hi) // 2
                candidate = cut(text, mid)
                tokens = count_tokens(url, candidate)
                out = candidate
                if abs(tokens - target) <= target * TOLERANCE:
                    break
                if tokens < target:
                    lo = mid
                else:
                    hi = mid
            report[name] = {"tokens": tokens, "chars": len(out), "files": files}
        with open(os.path.join(HERE, name), "w", encoding="utf-8", newline="\n") as fh:
            fh.write(out)
        print(name, report[name])
    with open(os.path.join(HERE, "prompts.json"), "w", encoding="utf-8") as fh:
        json.dump(report, fh, indent=2, ensure_ascii=False)


if __name__ == "__main__":
    main()
