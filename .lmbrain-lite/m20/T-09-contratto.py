"""M-20 T-09 — i tre endpoint che GalaxyCenter pretende, provati davvero.

Non prova «il servizio si accende»: prova che quello che esce dalle porte soddisfa il contratto
scritto in GalaxyCenter/.lmbrain-lite/knowledge/llm-server-contract.md, riga per riga.

  chat    POST /v1/chat/completions -> choices[0].message
  embed   POST /v1/embeddings       -> data[i].embedding, dimensione fissa
  rerank  POST /v1/rerank           -> results[i] con index e relevance_score
  tutti   GET  /health              -> 200
          GET  /v1/models           -> gli id che GalaxyCenter confronta

Sul rerank fa anche il test di sanita': il contratto dice che se il punteggio e' vicino a zero per
un documento pertinente, il GGUF e' rotto. Qwen non pubblica un GGUF ufficiale del reranker, quindi
questo e' un controllo che serve davvero, non una formalita'.

Uso: python T-09-contratto.py [--chat 8080] [--embed 8081] [--rerank 8082]
I servizi vanno accesi prima, con le righe che genera service::build_args.
"""

import argparse
import json
import sys
import urllib.error
import urllib.request

ESITI = []


def esito(ok, titolo, dettaglio=""):
    ESITI.append((ok, titolo, dettaglio))
    print(f"  {'OK  ' if ok else 'NO  '}{titolo}" + (f"  — {dettaglio}" if dettaglio else ""))


def get(url, timeout=30):
    with urllib.request.urlopen(url, timeout=timeout) as r:
        return r.status, json.loads(r.read() or b"{}")


def post(url, body, timeout=300):
    req = urllib.request.Request(url, json.dumps(body).encode(), {"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read())


def salute(nome, base):
    try:
        st, _ = get(f"{base}/health")
        esito(st == 200, f"{nome}: /health risponde 200", f"stato {st}")
    except Exception as e:
        esito(False, f"{nome}: /health risponde 200", str(e))
        return False
    try:
        _, v = get(f"{base}/v1/models")
        ids = [m.get("id") for m in v.get("data", [])]
        esito(bool(ids), f"{nome}: /v1/models espone un id", ", ".join(str(i) for i in ids))
    except Exception as e:
        esito(False, f"{nome}: /v1/models espone un id", str(e))
    return True


def prova_chat(base):
    if not salute("chat", base):
        return
    try:
        v = post(f"{base}/v1/chat/completions", {
            "messages": [{"role": "user", "content": "Rispondi con la sola parola: pronto."}],
            "max_tokens": 16, "temperature": 0.0,
            "chat_template_kwargs": {"enable_thinking": False},
        })
        msg = v["choices"][0]["message"]
        esito("content" in msg, "chat: choices[0].message ha un content", repr(msg.get("content", ""))[:60])
    except Exception as e:
        esito(False, "chat: choices[0].message ha un content", str(e))


def prova_embed(base, attese=1024):
    if not salute("embed", base):
        return
    try:
        v = post(f"{base}/v1/embeddings", {"input": ["prima frase", "seconda frase"]})
        dati = v.get("data", [])
        esito(len(dati) == 2, "embed: una riga per ogni input", f"{len(dati)} righe")
        dims = {len(d["embedding"]) for d in dati}
        esito(dims == {attese}, f"embed: dimensione fissa {attese}", f"trovate {sorted(dims)}")
        # Un vettore di soli zeri risponde al contratto e non serve a niente: va visto.
        somma = sum(abs(x) for x in dati[0]["embedding"])
        esito(somma > 0, "embed: il vettore non e' tutto zeri", f"somma dei valori assoluti {somma:.3f}")
    except Exception as e:
        esito(False, "embed: /v1/embeddings risponde secondo il contratto", str(e))


def prova_rerank(base):
    if not salute("rerank", base):
        return
    documenti = [
        "La ricetta della carbonara vuole guanciale, uovo e pecorino.",
        "Dopo il caricamento Aethera misura la VRAM dedicata e la RAM rimasta.",
        "Il treno per Milano parte dal binario nove alle sette e mezza.",
    ]
    try:
        v = post(f"{base}/v1/rerank", {
            "query": "Quanta memoria occupa il motore dopo il caricamento?",
            "documents": documenti,
        })
        res = v.get("results")
        esito(isinstance(res, list) and len(res) == 3, "rerank: results ha una riga per documento",
              f"{len(res) if isinstance(res, list) else '—'} righe")
        campi = all("index" in r and ("relevance_score" in r or "score" in r) for r in res)
        esito(campi, "rerank: ogni riga ha index e relevance_score")
        p = {r["index"]: r.get("relevance_score", r.get("score")) for r in res}
        pertinente, altri = p.get(1), [p.get(0), p.get(2)]
        esito(pertinente is not None and pertinente > 0.5,
              "rerank: TEST DI SANITA' — il documento pertinente prende un punteggio alto",
              f"pertinente {pertinente}, altri {altri}")
        esito(pertinente is not None and all(a is not None and pertinente > a for a in altri),
              "rerank: il pertinente stacca gli altri due")
    except Exception as e:
        esito(False, "rerank: /v1/rerank risponde secondo il contratto", str(e))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--chat", default="http://127.0.0.1:8080")
    ap.add_argument("--embed", default="http://127.0.0.1:8081")
    ap.add_argument("--rerank", default="http://127.0.0.1:8082")
    ap.add_argument("--embed-dim", type=int, default=1024)
    ap.add_argument("--salta", default="", help="chat,embed,rerank da saltare")
    a = ap.parse_args()
    salta = {x.strip() for x in a.salta.split(",") if x.strip()}

    if "chat" not in salta:
        print("\n— chat (motore principale)")
        prova_chat(a.chat)
    if "embed" not in salta:
        print("\n— embed (motore di servizio)")
        prova_embed(a.embed, a.embed_dim)
    if "rerank" not in salta:
        print("\n— rerank (motore di servizio)")
        prova_rerank(a.rerank)

    falliti = [t for ok, t, _ in ESITI if not ok]
    print(f"\n{len(ESITI) - len(falliti)}/{len(ESITI)} controlli passati")
    for t in falliti:
        print(f"  fallito: {t}")
    sys.exit(0 if not falliti else 2)


if __name__ == "__main__":
    main()
