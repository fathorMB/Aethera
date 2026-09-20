# La baseline del prodotto — 20 settembre 2026

Da oggi Aethera su questa macchina accende il motore in **un solo modo**, e i numeri qui sotto
sono il metro con cui si giudicherà ogni cambiamento futuro. Non è un'aspirazione: è la
configurazione che ha vinto le misure di M-15, M-16, M-17, M-18 e M-19, e i numeri sono quelli
dell'ultimo giro, non una media di comodo.

## Il profilo unico

`qwen3.6-35b-a3b.q8_0.vulkan` — Qwen3.6-35B-A3B Q8_0, Vulkan, llama.cpp b10809, contesto 262144,
`-ub 4096 -b 4096`, flash attention, KV f16, MTP con bozza 3, checkpoint del server spenti,
salvataggio degli slot acceso. Il ragionamento va **spento dal client** (`enable_thinking: false`):
non è una leva di questo file.

Gli altri 21 profili sono in `profiles/archivio/`. L'app legge solo i `.toml` che stanno in
`profiles/`, quindi non li vede più; per rimisurare una variante basta riportarne uno su e
rimetterlo giù dopo. Nessuna modifica al codice dell'app: l'installazione resta la v0.2.0.

## I numeri di riferimento

Batteria di coding agentico di M-15, 16 compiti, Nonio `main` `e15a6bc` (sha `20bc0cfb`),
contesto servito 32768, 20-09-2026:

| | |
|---|---|
| compiti risolti | **16 su 16** |
| tempo | 22,4 minuti |
| compiti riusciti per ora | **42,8** |
| turni / richieste al motore | 143 / 143 |
| riuso del prefisso | **90,3 %** |
| prefill per richiesta | 1,61 s |
| costo fisso (richiesta che non aggiunge quasi niente) | 0,42 s |
| decode | ~25,8 tok/s |
| accettazione della bozza MTP | 81,4 % |
| richieste che riusano zero | **una per compito** — la prima, su cartella nuova |

**Quando un risultato è davvero peggiore.** Fra giri identici il rumore misurato è di 1-2 compiti
e circa il 10% sui compiti per ora (M-18 T-02), e il numero di turni oscilla anche di più: 104
contro 143 nello stesso A-B. Sotto quelle soglie non è una differenza, è rumore.

## Il riferimento continuo: le sessioni vere

Ogni avvio del motore lascia `runs/<id>/` con `manifest.toml` (profilo, build, riga di comando,
contesto servito), `server.log` e `telemetry.jsonl` (una riga per richiesta). Sono quelli il
riferimento da qui in avanti, non un banco rifatto ogni volta:

```
python .lmbrain-lite/batteria/baseline.py --ultime 5
```

stampa per ogni sessione riuso, prefill per richiesta, costo fisso, decode, accettazione della
bozza e **richieste senza riuso**, accanto ai numeri di questa pagina. Lo strumento è stato
verificato contro i due bracci dell'A-B di M-18 T-14: ricava 90,3% e 83,4% di riuso e 16 e 17
richieste senza riuso, cioè esattamente quello che aveva calcolato il runner della batteria per
un'altra strada.

Il numero da sorvegliare è **le richieste senza riuso**. Il riuso del prefisso è tutto-o-niente:
`llama-server` riusa solo se il prompt nuovo è un'estensione esatta di quello in cache, e alla
prima divergenza riparte da zero, non in parte (misura del 20-09). Una richiesta senza riuso per
conversazione nuova è il costo d'ingresso; più di una vuol dire che il client sta facendo
divergere il prompt, e a 262144 quello costa un prefill intero.

## Cosa resta fuori, e si sa perché

- **Ling-3.0-flash**: 4,6 compiti/ora contro 34,9. Scartato (`reports/ling-3.0-flash-2026-09.md`).
- **Ragionamento acceso**: costa un terzo della produttività e non risolve un compito in più.
- **Speculativa a n-grammi**: sul lavoro vero il G1 perde un quarto contro MTP.
- **Q6_K**: non porta niente rispetto al Q4, e il Q8 batte entrambi.
- **Checkpoint accesi**: a 262144 costano ~1,6 s per richiesta senza comprare riuso; a 32768 non
  costano niente (0,42 contro 0,43 s). Il loro costo cresce col contesto, e la divergenza da cui
  proteggevano è chiusa in Nonio (`78b4099`).

## Anche le installazioni nuove nascono così

Deciso dall'operatore il 20-09: il profilo non è più uno stato di questa macchina, è il
predefinito del prodotto. Aethera lo scrive in `profiles/` alla prima esecuzione, con la stessa
regola dei template — solo se quel file non c'è già, così chi lo modifica se lo tiene, e chi lo
cancella se lo ritrova al prossimo avvio. Sorgente in `src-tauri/profiles/`, semina in
`DataRoot::ensure`, con un test che copre tutti e due i casi.

Su una macchina nuova mancano i pesi (37,8 GB) e la build di llama.cpp: l'app lo segnala e il
catalogo li scarica verificando lo SHA-256 dichiarato nel profilo. Il contesto 262144 chiede
VGM 64; a VGM 48 il tetto misurato è 131072.
