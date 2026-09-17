# Parere del critico su M-17 e M-18

> 18-09-2026 notte. Sub-agent in sola lettura (repository, JSONL di `<radice>\m15\risultati`,
> `llama-server --help` di b10991+moro1, ricerca web). Chiesto dall'operatore dopo l'analisi dei
> punti deboli del piano. «Verificato» = letto nei file; «supposto» = non verificato.

## 1. Punti ciechi, in ordine di gravità

**1. Il piano ottimizza a 32k, il requisito è 128-256k.**
- Verificato: nelle 105 righe della batteria il contesto massimo raggiunto è 21.464 token.
- Verificato (`reports/misure-motore-2026-09.md:157-158`): MTP a 21k fa già perdere decode al G1,
  16,29 contro 19,22 senza speculazione (−15 %). Il vantaggio del G1 sul G3 può annullarsi in
  profondità.
- Dalla lettura del codice: con MTP ogni checkpoint copia tutta la KV del draft (~2 KiB/token):
  a 256k ~0,5 GiB, due volte per richiesta.
- `--ctx-checkpoints 0`, la leva più ovvia di M-17 T-05, trasforma ogni divergenza in un ricalcolo
  completo: a 128k è un disastro, e M-17 T-07 (5 compiti a 32k) non può accorgersene.
- Correzione: M-18 T-09 al primo posto; ogni leva di M-17 misurata anche a 64k e 128k e con una
  divergenza provocata; MTP acceso e spento in profondità.

**2. Il decode pesa il 69-73 % del tempo e il piano non lo tocca.**
- Ricalcolato dai JSONL di notte-1: G1 decode 69 %, prefill 26 %; G3 decode 73 %, prefill 24 %.
- Il costo fisso (1,5 s × richieste) vale 10-12 % sul G3 e **19 %** sul G1 (242 richieste su
  1.925 s).
- Verificato da `--help`: `--spec-type` accetta `ngram-simple`, `ngram-map-k`, `ngram-mod`,
  `ngram-cache`, `draft-simple`.
- Supposto: un agente che riscrive pezzi di file è il caso favorevole per gli n-grammi; per il G3
  il draft naturale è un Qwen3 piccolo con lo stesso tokenizer.
- Correzione: speculativa a n-grammi sul G3, n-grammi insieme a MTP sul G1, MTP in profondità;
  prima della scala dei quant.

**3. La batteria non può decidere la scala dei quant.**
- Verificato (`reports/int8-coopmat-2026-09.md:280-282`): fra due giri uguali cambiano 1-4
  compiti, 9 su 15 danno sempre lo stesso esito. G3: 12, 11, 10, 10, 10 su tre configurazioni; il
  «G1 = G3 a 12/15» poggia su un giro solo.
- Con 3 giri servono ~3 compiti di scarto per vedere una differenza.
- Correzione: prima la KLD di Q4 e Q6 contro Q8 con gli script di M-16; batteria solo se la KLD
  supera il metro dell'ubatch.
- M-18 T-02 sbaglia il riferimento del G3: il profilo standard è passato a b10991+moro1 con
  ubatch 2048, notte-1 non conta come primo giro.

**4. La deriva della macchina è peggiore del −13 %.**
- Verificato (`LOG.md`, 17-09 22:39 e 22:46): il G3 perde metà del prefill (134 contro 286) e il
  19 % del decode, con più spill in condivisa; dopo il riavvio torna.
- Supposto: il «disturbo non identificato» di int8-1 è lo stesso fenomeno.
- Correzione: sentinella (8B tg128 più un prefill breve sul G3) prima e dopo ogni blocco, blocco
  scartato se cala oltre il 3 %; confronti alternati ABAB; riavvio prima di ogni notte. Le leve di
  M-18 T-01 valgono meno della deriva.

**5. Thinking: costo nascosto confermato in parte.**
- Supposto: dentro una catena di chiamate agli strumenti i template Qwen3 conservano il
  ragionamento dopo l'ultimo messaggio utente, ma solo se il client rimanda `reasoning_content`:
  per Nonio va verificato.
- Dal codice: se il checkpoint a n−4 esiste il danno è riprocessare il messaggio dell'assistente;
  se manca, tutto.
- Issue llama.cpp #22615 e #23030 (letti solo i titoli): `preserve_thinking` non ripristina il
  riuso su qwen35. Da leggere prima di M-18 T-05.

Altri modelli: una ricerca rapida non mostra candidati chiaramente migliori a 48-64 GB; uno solo,
e solo dopo la correzione di Nonio. Dispersione: vera, vedi «da togliere».

## 2. Che cosa è solido

M-17 T-01…T-04 (metro sul denso 8B, `/completion` contro chat, log a `-lv 5`); la deduzione
«18 + 4 con due checkpoint», riletta nel sorgente (`server-context.cpp` 3560-3634); i difetti di
Nonio come leva più economica (26 compiti su 45 arrivano al tetto, 15 erano già risolti); la regola
di fedeltà di M-16; le regole per l'harness; i profili standard lasciati all'operatore.

## 3. Ordine consigliato a VGM 48

- **Notte 1** (riavvio, sentinella): M-17 T-01…T-03 con `-lv 5`; T-04 esteso a 64k e 128k;
  speculativa a n-grammi sul G3; MTP acceso e spento sul G1 a 32k e 128k; KLD di Q4 e Q6 contro Q8.
- **Notte 2**: costo per turno in profondità con G1 e G3 alternati; divergenza provocata con
  checkpoint a 0 e a 32; salvataggio e ripristino dello slot.
- **Da togliere o rimandare**: M-18 T-01 (BIOS e servizi); la batteria della scala dei quant;
  T-05 finché non si sa che cosa fa Nonio con il ragionamento; T-10 finché Nonio non è corretto;
  T-06 e FN (8-9 tok/s e un compito in 407 s contro 41 bastano come verdetto); FC Q8_0. Chiudere
  M-10 e M-11 con un no. Tag v0.1.0 e prove dell'operatore non dipendono da niente di questo.

## 4. Errori di fatto trovati

- `int8-coopmat-2026-09.md` riga 23 «Batteria (un giro): due giri»; riga 25 «1–3 compiti» contro
  «1 e 4» della sezione 3.2.
- M-18 T-03 chiama il costo fisso «la leva di velocità più grande nota»: è una deduzione. Il costo
  fisso è l'intercetta di una regressione dominata da una decina di richieste grandi; può essere
  il regime MoE a lotto piccolo (22 token a ~69 ms l'uno) e non un costo recuperabile. Stima del
  critico, non misura: 75 MiB e 72 submit per checkpoint non sembrano arrivare a 1,5 s.
- `int8-coopmat-2026-09.md` righe 289-294: py-slug e py-ledger falliscono 4 giri su 4 con la patch
  e riescono 5 su 5 senza; liquidato come coincidenza. Con pochi giri non prova niente in nessuno
  dei due sensi, ma la patch è già nel profilo standard.
- `M-15.md` e il rapporto della batteria dicono «FN non avviato»: il JSONL di notte-1 contiene 2
  righe di FN (sono quelle del 17-09 sera, dopo il rapporto).

Fonti web: issue llama.cpp #22615, #23030, #22746; InsiderLLM «best local coding models 2026».
