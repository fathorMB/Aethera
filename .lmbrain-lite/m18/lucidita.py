"""M-18 T-09: il modello resta lucido a contesto lungo?

Misura diretta: si costruisce una conversazione che cresce in coda fino a una profondità voluta
(32k, 64k, 128k, 200k token), dentro ci si semina un fatto preciso a una posizione nota — una riga
di codice con un valore inventato, in mezzo a codice vero — e alla fine si chiede quel fatto.
Il modello risponde giusto o no: è un sì/no, ripetuto su più semi e più profondità.

Non è un test di coding: dice solo se, a quella profondità, il modello trova ancora quello che ha
davanti. Se non lo trova, nessuna misura di velocità a contesto lungo ha senso.

Tre posizioni per profondità: il fatto messo al 10 %, al 50 % e al 90 % della conversazione. La
letteratura dice che il mezzo è il punto debole, e qui si verifica sulla nostra macchina.

Uso:
  python lucidita.py --profilo qwen3.6-35b-a3b.q4_k_m.vulkan --nome g1 --profondita 32000 128000
Uscite in <radice>/m18/lucidita.jsonl e lucidita-<nome>.log.
"""

import argparse
import json
import os
import random
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

QUI = Path(__file__).resolve().parent
sys.path.insert(0, str(QUI.parent / "batteria"))
import batteria as b  # noqa: E402
import esegui as e  # noqa: E402

# Il fatto seminato: una costante con un nome plausibile e un valore che non si può indovinare.
NOMI = ["RETRY_BACKOFF_MS", "MAX_SHARD_BYTES", "FLUSH_WATERMARK", "LEASE_RENEW_MS",
        "COMPACTION_FANOUT", "SNAPSHOT_STRIDE", "WAL_SEGMENT_KIB", "REPLICA_LAG_MS"]
SISTEMA = ("You are reading a large codebase with the developer. Answer questions about it "
           "precisely and briefly, using only what you have been shown.")


def http_json(url: str, corpo: dict, timeout: float = 3600) -> tuple[dict, float]:
    req = urllib.request.Request(url, data=json.dumps(corpo).encode("utf-8"),
                                 headers={"Content-Type": "application/json"})
    t0 = time.perf_counter()
    with urllib.request.urlopen(req, timeout=timeout) as r:
        testo = r.read()
    return json.loads(testo), (time.perf_counter() - t0) * 1000


def pezzi(sorgente: str, quanti: int, per_pezzo: int) -> list[str]:
    """Spezza il sorgente in blocchi grandi come un file letto da un agente."""
    out, i = [], 0
    while len(out) < quanti and i + per_pezzo <= len(sorgente):
        out.append(sorgente[i:i + per_pezzo])
        i += per_pezzo
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--profilo", required=True)
    ap.add_argument("--nome", required=True)
    ap.add_argument("--profondita", type=int, nargs="+", default=[32000, 64000, 128000])
    ap.add_argument("--posizioni", type=float, nargs="+", default=[0.1, 0.5, 0.9])
    ap.add_argument("--semi", type=int, default=2, help="quante volte ripetere ogni combinazione")
    # Il sorgente congelato lo scrive sorgente-lungo.py nella radice dati, non nel repository.
    ap.add_argument("--sorgente", default=str(b.radice() / "m18" / "sorgente-lungo.txt"))
    ap.add_argument("--ctx", type=int, default=262144)
    ap.add_argument("--extra")
    ap.add_argument("--gia-acceso")
    args = ap.parse_args()

    radice = b.radice()
    out = radice / "m18"
    out.mkdir(parents=True, exist_ok=True)
    sorgente = Path(args.sorgente).read_text(encoding="utf-8")
    diario = open(out / f"lucidita-{args.nome}.log", "a", encoding="utf-8")

    def scrivi(m: str) -> None:
        print(m, flush=True)
        diario.write(m + "\n")
        diario.flush()

    pronto_f = Path(args.gia_acceso) if args.gia_acceso else out / f"lucidita-{args.nome}.pronto.json"
    proc = None
    if not args.gia_acceso:
        while True:
            o = e.ostacoli(radice, e.RAM_MINIMA_GIB)
            if not o:
                break
            scrivi("aspetto: " + "; ".join(o))
            time.sleep(60)
        pronto_f.unlink(missing_ok=True)
        (radice / "stop-m15").unlink(missing_ok=True)
        exe = os.environ.get("AETHERA_M15_HOLD") or str(
            QUI.parents[1] / "src-tauri" / "target" / "release" / "examples" / "m15_hold.exe")
        cmd = [exe, str(radice), args.profilo, "--ctx", str(args.ctx), "--pronto", str(pronto_f)]
        if args.extra:
            cmd += ["--extra", args.extra]
        proc = subprocess.Popen(cmd, stdout=open(out / f"hold-lucidita-{args.nome}.log", "a", encoding="utf-8"),
                                stderr=subprocess.STDOUT)
        t0 = time.monotonic()
        while not pronto_f.is_file():
            if proc.poll() is not None:
                scrivi(f"m15_hold è uscito con {proc.returncode}")
                return 1
            if time.monotonic() - t0 > 1800:
                proc.kill()
                return 1
            time.sleep(1)
    pronto = json.loads(pronto_f.read_text(encoding="utf-8"))
    base = pronto["base_url"].rstrip("/")
    scrivi(f"=== {args.nome} · {pronto['run_id']} · ctx {pronto.get('ctx_served')} · RAM libera {e.ram_libera_gib():.1f} GiB")

    esiti = []
    try:
        with open(out / "lucidita.jsonl", "a", encoding="utf-8") as fh:
            for prof in args.profondita:
                # ~3,3 caratteri per token; blocchi da ~1.500 token, come un file letto da un agente
                per_pezzo = 5000
                quanti = max(4, int(prof * 3.3 / per_pezzo))
                for pos in args.posizioni:
                    for seme in range(args.semi):
                        rnd = random.Random(hash((prof, pos, seme)) & 0xFFFF)
                        nome = rnd.choice(NOMI)
                        valore = rnd.randrange(1000, 9999)
                        salto = rnd.randrange(0, max(1, len(sorgente) - quanti * per_pezzo - 1))
                        blocchi = pezzi(sorgente[salto:], quanti, per_pezzo)
                        dove = min(len(blocchi) - 1, max(0, int(len(blocchi) * pos)))
                        ago = (f"\n\n# Project constant, set after the 2026 capacity review:\n"
                               f"{nome} = {valore}  # do not change without asking the storage team\n\n")
                        blocchi[dove] = blocchi[dove][:len(blocchi[dove]) // 2] + ago + blocchi[dove][len(blocchi[dove]) // 2:]
                        messaggi = [{"role": "system", "content": SISTEMA}]
                        for i, blk in enumerate(blocchi):
                            messaggi.append({"role": "user", "content": f"Here is part {i + 1} of the codebase:\n\n{blk}"})
                            messaggi.append({"role": "assistant", "content": f"Noted part {i + 1}."})
                        messaggi.append({"role": "user", "content":
                                         f"In the code you have been shown there is a constant named {nome}. "
                                         f"What is its value? Answer with the number only."})
                        corpo = {"model": pronto["alias"], "messages": messaggi, "max_tokens": 16,
                                 "temperature": 0.0, "seed": 1234, "stream": False, "cache_prompt": True,
                                 "chat_template_kwargs": {"enable_thinking": False}}
                        try:
                            risposta, muro = http_json(base + "/v1/chat/completions", corpo)
                        except Exception as exc:  # una profondità che non ci sta lo dice e si va avanti
                            scrivi(f"  {prof//1000}k pos {pos} seme {seme}: ERRORE {exc}")
                            fh.write(json.dumps({"variante": args.nome, "profondita": prof, "posizione": pos,
                                                 "seme": seme, "errore": str(exc)}) + "\n")
                            continue
                        testo = ((risposta.get("choices") or [{}])[0].get("message", {}).get("content") or "").strip()
                        t = risposta.get("timings") or {}
                        giusto = str(valore) in testo
                        r = {"variante": args.nome, "run_id": pronto["run_id"], "profondita": prof,
                             "posizione": pos, "seme": seme, "costante": nome, "atteso": valore,
                             "risposta": testo[:80], "giusto": giusto,
                             "prompt_n": t.get("prompt_n"), "cache_n": t.get("cache_n"),
                             "prompt_ms": t.get("prompt_ms"), "muro_ms": round(muro, 1)}
                        fh.write(json.dumps(r, ensure_ascii=False) + "\n")
                        fh.flush()
                        esiti.append(r)
                        scrivi(f"  {prof//1000:3d}k pos {pos} seme {seme}: {'OK ' if giusto else 'NO '} "
                               f"«{testo[:30]}» atteso {valore} · prompt {t.get('prompt_n')} token in "
                               f"{(t.get('prompt_ms') or 0)/1000:.1f} s")
    finally:
        if proc is not None:
            (radice / "stop-m15").write_text("", encoding="utf-8")
            try:
                proc.wait(timeout=180)
            except subprocess.TimeoutExpired:
                proc.kill()

    scrivi("\n=== riepilogo")
    for prof in args.profondita:
        for pos in args.posizioni:
            v = [x for x in esiti if x["profondita"] == prof and x["posizione"] == pos]
            if v:
                scrivi(f"  {prof//1000:3d}k pos {pos}: {sum(x['giusto'] for x in v)}/{len(v)} giusti")
    return 0


if __name__ == "__main__":
    sys.exit(main())
