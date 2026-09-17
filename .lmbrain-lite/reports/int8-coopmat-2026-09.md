# int8 coopmat: fedeltà numerica, ubatch e compiti riusciti

> **M-16, 17-09-2026.** Misure dalle 13:49 alle 20:37, Windows 26200.9457, VGM 48, Smart App
> Control spento (ultimi eventi CodeIntegrity 3033/3077 alle 07:46, prima dello spegnimento), driver
> invariati. Build `b10991+moro0` (tag liscio compilato qui) e `b10991+moro1` (più la PR
> ggml-org/llama.cpp#27952, commit 8253abef6), le stesse di M-14. Sigle in fondo.

**In breve.**
- **La patch regge, per G3 e per G1.**
  - La perplessità non peggiora su nessuno dei due.
  - La divergenza KL dalla base è dell'ordine di quella fra due calcoli corretti: sul G3 1,4 volte
    quella del solo cambio di ubatch; sul G1 più piccola di quella fra il backend CPU e Vulkan.
- **Il picco di KLD del G1 (12,1) non è un errore del kernel int8.** Cade in una zona fragile del
  prompt dove anche la CPU diverge dalla base. Lì la patch segue il testo vero meglio della base
  (PPL 2,75 contro 3,53) e della CPU (5,00). Sezione 1.3.
- **L'ubatch migliore:**
  - per il G3 è **2048** con e senza patch: prefill a 7k da 183 (profilo di oggi: ubatch 512,
    base) a **286 tok/s** (+57 %);
  - per il G1 resta **4096**, e la patch aggiunge solo il 4 %.
- **Cambiare solo l'ubatch cambia già il testo a temperatura 0**, su G1 e G3, spesso allo stesso
  carattere dove diverge la patch. La regola «stesso testo» era mal posta: è riscritta con soglie di
  KLD nel README del fork (sezione 4).
- **Batteria (un giro):** **due giri per configurazione.** G3 con la patch 10 e 10 compiti su 15;
  il controllo senza patch, stessa base e stesso ubatch, 11 e 10. La differenza è dentro il rumore
  (fra due giri della stessa configurazione cambiano 1–3 compiti), nessun file protetto è stato
  toccato e non compaiono cause di fallimento nuove. **Nel lavoro con l'agente il tempo non
  migliora**: il prefill effettivo passa da 113 a 122 tok/s (+8 %), non +35 %, perché ogni richiesta
  paga circa 1,5 s fissi e porta solo 314 token nuovi (sezione 3.3)
- **Un prompt a freddo da 16.822 token** (il prompt fisso di Claude Code) sul G3 passa da circa
  **104 s a 69 s** (−35 s). Sul G1 da 47,8 a 46,1 s.

## 1. Fedeltà numerica (T-01)

### 1.1 Come

`llama-perplexity` con le opzioni di `run-T06-perplexity.sh` di M-14:
- prompt congelato da 21k (`.lmbrain-lite/m08/prompt-21k.txt`), contesto 8192: 2 blocchi, 8.190
  token valutati (la seconda metà di ogni blocco);
- `-ngl 999 -fa on`, cache f16, ubatch e batch del profilo (G1 4096/4096, G3 512/2048).

Script `.lmbrain-lite/m16/sequenza.py`, fasi `kld-g1`, `kld-g3`, `picco`, `cpu-g1`; uscite in
`<radice>\m16\T-01-*.txt` e `picco-*.txt`. La base (moro0) scrive i logit con
`--kl-divergence-base`, e le altre varianti si confrontano con `--kl-divergence`.

**Il file dei logit** usa 2 byte per voce (log-probabilità quantizzate a 16 bit), non 4:
- G1 (vocabolario 248.320): 3,79 GiB;
- G3: 2,32 GiB.

Tutti i file sono sotto i 10 GB, quindi i token non sono stati ridotti. Per l'analisi token per
token (1.3) sono stati salvati come «base» anche moro1, la base con l'altro ubatch e, per il G1, il
backend CPU: in tutto 7 file e 22,1 GiB, cancellati alla fine (1.4).

**Metri:**
- **pavimento**: la base contro se stessa;
- **metro dell'ubatch**: la base con un altro ubatch (G1 512 invece di 4096; G3 2048 invece di 512);
- **metro del backend** (solo G1): la base calcolata dalla CPU (`-dev none -ngl 0`, 111 s per
  blocco). Il G3 non ci sta: 48,5 GB di pesi contro 34 GiB di RAM libera.

### 1.2 Risultati

«Stesso primo token» è il «Same top p» di llama-perplexity. ΔPPL è la differenza di perplessità
dalla base, con il suo errore. Le righe CPU vengono da `kld_per_token.py` (la stessa formula,
calcolata sui due file).

**G1** (35B-A3B; base PPL 2,2601 ± 0,0456):

| confronto con la base | KLD media | KLD 99 % | KLD max | stesso primo token | ΔPPL |
|---|---:|---:|---:|---:|---:|
| base ripetuta (pavimento) | 0,000000 | 0,00003 | 0,00006 | 100,00 % | 0,0000 ± 0,0000 |
| base, ubatch 512 (metro) | 0,00066 | 0,0064 | 0,020 | 98,89 ± 0,12 % | −0,0001 ± 0,0010 |
| **patch, ubatch 4096** | **0,0102** | **0,066** | **12,19** | **97,94 ± 0,16 %** | **−0,0044 ± 0,0054** |
| patch, ubatch 512 | 0,0101 | 0,066 | 12,12 | 98,05 ± 0,15 % | −0,0049 ± 0,0054 |
| CPU (metro del backend) | 0,0180 | 0,101 | 12,81 | 97,51 % | +0,0164 (PPL 2,2765 ± 0,0464) |
| CPU contro patch (non contro la base) | 0,0168 | 0,094 | 12,81 | 97,42 % | |

**G3** (Coder-Next; base PPL 2,7182 ± 0,0667):

| confronto con la base | KLD media | KLD 99 % | KLD max | stesso primo token | ΔPPL |
|---|---:|---:|---:|---:|---:|
| base ripetuta (pavimento) | 0,000000 | 0,00000 | 0,00000 | 100,00 % | +0,0017 ± 0,0010 ¹ |
| base, ubatch 2048 (metro) | 0,0065 | 0,082 | 0,88 | 97,19 ± 0,18 % | +0,0000 ± 0,0052 |
| **patch, ubatch 512** | **0,0092** | **0,113** | **3,71** | **97,11 ± 0,19 %** | **−0,0032 ± 0,0056** |
| patch, ubatch 2048 | 0,0091 | 0,117 | 2,97 | 97,18 ± 0,18 % | −0,0020 ± 0,0054 |

¹ Il pavimento del ΔPPL è l'arrotondamento a 16 bit della log-probabilità della base nel file.

**Come leggerli:**
- **La base è deterministica**: si ripete fino all'ultima cifra, come in M-14.
- **Sul G3 cambiare solo l'ubatch sposta già molto le distribuzioni** (G3 è ibrido: i kernel dello
  stato ricorrente dipendono dal taglio del lotto). La patch sta a 1,4 volte il metro nella media
  e nel 99 %, con lo stesso numero di primi token uguali.
- **Sul G1 il metro dell'ubatch è minuscolo**, perché i kernel restano gli stessi. La patch vale
  15 volte quel metro, ma cambia il percorso numerico (prodotto in int8 invece che in float a
  16 bit), e va confrontata con un altro backend corretto: la CPU si discosta dalla base più della
  patch (0,018 contro 0,010) ed è più lontana dalla base che dalla patch.
- **La perplessità con la patch scende di poco** in tutti e quattro i casi, dentro l'errore.

### 1.3 Il picco del G1

Sul G1 la patch ha una KLD massima di 12,2, con tutti e due gli ubatch, contro 0,02 del metro.
Analisi token per token (`kld_per_token.py`, `nll_zona.py`; uscite in
`<radice>\m16\picco-*-per-token.txt`, `nll-zone-*.txt`).

**Non è un token solo.** Patch contro base:
- 8 token sopra 1,0 e 18 sopra 0,5, su 8.190;
- 7 gruppi, tutti nel blocco 0. Il più grande sta nelle posizioni 5707–5770: 10 token, in una
  struct Rust piena di `#[serde(default, skip_serializing_if = "Option::is_none")]`.

**Nei token peggiori sbaglia la base, non la patch:**

| pos | contesto | base | patch | token vero |
|---|---|---|---|---|
| 5736 | `#[serde(default,` | « un» 0,90; « skip» 0,000 | « skip» 0,994 | « skip» |
| 5770 | `skip_serializing_if = "` | «0» 0,66; «Option» 0,06 | «Option» 0,975 | «Option» |
| 4764 | `server(32_768, "f16", "f` | «6» 0,94; «1» 0,02 | «1» 0,997 | «1» |
| 7389 | rientro dopo `return Vec::new();` | «       » 0,82 | «   » 0,90 | «       » |

Nell'ultima riga ha ragione la base. Negli altri casi la base dà probabilità quasi nulla a un token
che il contesto rende ovvio.

**Il riferimento CPU.** Nella stessa zona anche il backend CPU diverge, e più della base: in
posizione 5745 predice « Slots» (0,17), dove la patch dà «_none» con p = 1,000. Tutti e 7 i gruppi
della patch compaiono anche in CPU contro base (10 gruppi, 20 token sopra 1,0). NLL media del
token vero, zona per zona:

| zona (posizioni) | token | CPU | base ub 4096 | base ub 512 | patch ub 4096 |
|---|---:|---:|---:|---:|---:|
| 5680–5773 (il gruppo) | 94 | 1,610 | 1,262 | 1,264 | **1,011** |
| 4764 | 1 | **0,000** | 4,075 | 3,934 | **0,003** |
| 5976–6011 | 36 | 0,127 | **0,068** | 0,072 | 0,141 |
| blocco 0 (4096–8190) | 4.095 | 0,719 | 0,704 | 0,705 | **0,700** |
| blocco 1 (12288–16382) | 4.095 | 0,926 | 0,926 | 0,925 | 0,927 |

**Conclusione.** Nel blocco 1 i quattro calcoli coincidono. Nel blocco 0 c'è una zona in cui il
modello è mal condizionato e ogni implementazione dà un risultato diverso: è una proprietà del
modello su quel testo, non del kernel int8. Lì la patch è la più vicina al testo vero, e in
posizione 4764 concorda con la CPU contro la base. Le due basi Vulkan (ubatch 4096 e 512) sbagliano
allo stesso modo. Un'ipotesi non verificata: il prodotto in float a 16 bit del percorso di base,
dove quello int8 lavora con scale a 32 bit per blocco. Il 35B ha molti tensori Q8_0 e Q6_K
(attenzione, esperti, shared expert, `ssm_out`); il G3 nessun Q8_0.

**Sul G3** i picchi sono token incerti per natura (nomi inventati come `--questa-leva`), e il
metro dell'ubatch colpisce gli stessi punti (la posizione 12587 è la prima in tutti e due). Nella
zona 15000–15130, quella del massimo della patch, la NLL è la stessa: base 1,015, patch 1,018.

### 1.4 File

Cancellati alla fine, perché si rifanno in pochi minuti con `sequenza.py`: i 7 file dei logit in
`<radice>\m16\*.kld` (22,1 GiB). Restano le uscite testuali.

## 2. Ubatch con e senza patch (T-02)

`m08_bench` compilato nel worktree, scenari `.lmbrain-lite/m16/T-02a.toml` (G1) e `T-02b.toml`
(G3):
- 8 varianti per modello: ubatch 512, 1024, 2048 e 4096, base e patch una dopo l'altra per ogni
  ubatch;
- 1 riscaldamento e 3 giri, prompt congelati 7k e 21k, temperatura 0, seme 1234, nonce fisso
  per giro, testo intero;
- batch: G1 4096 fisso (quello del profilo); G3 max(2048, ubatch);
- G1 con MTP a 3 token come il profilo; profili standard non toccati (build e server per variante).

Nuova opzione `--riprendi` nel banco: salta le varianti già complete nel JSONL. Righe in
`<radice>\m16\T-02a.jsonl` e `T-02b.jsonl`; tabelle con
`python .lmbrain-lite/m16/analisi.py banco <file>`.

**Durata: 16 avvii.**
- G1: 60 minuti (14:03–15:03), circa 7 minuti per avvio con caricamento da 7 s.
- G3: 90 minuti (15:11–16:40), 11 minuti per avvio con caricamento da 32 s.

Mediana (sd) in tok/s.

**G1** (MTP 3; prompt 7.097 token; memoria dopo il caricamento):

| ubatch | 7k base | 7k patch | Δ | 21k base | 21k patch | Δ | decode 7k base / patch | VRAM |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 512 | 317,8 (1,1) | 378,3 (0,5) | +19,0 % | 254,0 | 294,3 | +15,9 % | 31,1 / 29,2 | 21,9 GiB |
| 1024 | 374,0 (2,0) | 428,9 (5,2) | +14,7 % | 293,7 | 326,8 | +11,3 % | 30,0 / 30,5 | 22,1 GiB |
| 2048 | 418,1 (1,4) | 453,2 (0,6) | +8,4 % | 328,2 | 350,9 | +6,9 % | 28,0 / 30,4 | 22,5 GiB |
| **4096** | **443,3 (1,1)** | **460,9 (1,0)** | **+4,0 %** | **351,7** | **364,6** | **+3,7 %** | 32,1 / 30,7 | 23,2 GiB |

**G3** (senza speculazione; prompt 6.754 token):

| ubatch | 7k base | 7k patch | Δ | 21k base | 21k patch | Δ | decode 7k | VRAM |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 512 (profilo) | 182,6 (0,2) | 243,2 (0,2) | +33,2 % | 162,0 | 207,2 | +27,9 % | 19,96 / 19,93 | 46,37 GiB |
| 1024 | 207,6 (0,6) | 276,8 (0,2) | +33,3 % | 182,6 | 232,3 | +27,2 % | 19,95 / 19,96 | 46,50 GiB |
| **2048** | **219,1 (1,5)** | **286,2 (0,2)** | **+30,6 %** | **193,5** | **243,8** | **+26,0 %** | 19,94 / 19,95 | 46,73 GiB |
| 4096 | 199,4 (0,2) | 270,0 (0,2) | +35,4 % | 171,9 | 228,8 | +33,1 % | 19,95 / 19,95 | 47,16 GiB |

**Che cosa dicono:**
- **L'ipotesi del milestone è confermata a metà.** La patch vale di più a ubatch piccoli (G1: +19 %
  a 512, +4 % a 4096). Ma per il G1 un ubatch più piccolo non conviene nemmeno con la patch:
  4096 resta il migliore (461 contro 453 a 2048).
- **Sul G3 l'ubatch 2048 batte il 512 del profilo** anche senza patch (+20 % a 7k). A 4096 si
  rallenta: la VRAM arriva a 47,2 GiB su 48 e il buffer di calcolo esce dalla VGM.
- **G3, combinazione migliore:** patch con ubatch 2048, **+57 % a 7k e +50 % a 21k** sul profilo
  di oggi con la stessa base (ubatch 512).
- **Decode, memoria e caricamento** non cambiano con la patch. Sul G1 il decode oscilla con MTP
  (sd 2–3 tok/s, accettazione 0,66–0,81), su tutte e due le build.
- **21k, un difetto del banco.** Con il nonce fisso, il 21k di un giro condivide l'inizio con il 7k
  dello stesso giro, e il server riusa un checkpoint a n−4−ubatch (M-15, sezione 6): i token
  elaborati vanno da 13.604–14.099 (ubatch 512) a 17.184–17.679 (4096). Base e patch allo stesso
  ubatch si confrontano bene; fra ubatch diversi fa fede il 7k, sempre a freddo (6.754 e 7.097
  token).

### Il testo a temperatura 0

Nonce e seme uguali, giro per giro (3 giri per carico):

| confronto | G1 7k | G1 21k | G3 7k | G3 21k |
|---|---|---|---|---|
| base ubatch X contro base ubatch del profilo (3 coppie) | 0/3 in tutte e 3 | 0/3 in tutte e 3 | 0/3, 0/3, 1/3 | 0/3 in tutte e 3 |
| patch contro base, stesso ubatch (4 coppie) | 0/3 in tutte e 4 | 0/3 in tutte e 4 | 0/3 in tutte e 4 | 0/3 in tutte e 4 |

I punti di divergenza si somigliano:
- G1 7k: la base con ubatch 1024 o 2048 diverge dalla base 4096 anche al carattere 8, come la
  patch;
- G3 7k: tutte le coppie divergono ai caratteri 150–167, patch o ubatch che sia.

**Cambiare solo l'ubatch è una modifica numerica della stessa natura della patch.** Sul G1 le
distribuzioni cambiano meno (1.2), ma il testo cambia lo stesso: a temperatura 0 basta spostare il
primo token incerto.

## 3. Batteria (T-03)

### 3.1 Come

`esegui.py` di M-15 con l'orchestrazione `.lmbrain-lite/m16/batteria.py`, sessioni nuove
(`int8-1`, `int8-2`), stessi 15 compiti e stessi tetti di `notte-1`, un giro per sessione, Nonio
`C:\Git\Nonio\target\release\nonio.exe` (lo stesso binario di `notte-1`, invariato dal 15-09).

Due configurazioni, tutte e due su profili di prova (i profili standard non sono toccati):
- **G3-int8**: `qwen3-coder-next.q4_k_m.vulkan.int8` — `b10991+moro1`, ubatch 2048;
- **G3-moro0**: `qwen3-coder-next.q4_k_m.vulkan.moro0` — `b10991+moro0`, ubatch 2048.

Il controllo serve perché `notte-1` girava su **b10809 con ubatch 512**: senza di esso la patch non
si separa dal tag e dall'ubatch.

**Perché due giri.** Nel primo (`int8-1`, la patch per prima) il giro con la patch è andato più
piano del previsto per 45 minuti: costo marginale del prefill 141–160 tok/s invece dei circa 300
attesi, mentre il controllo, subito dopo, stava a 225–239. Non si è capito che cosa disturbasse la
macchina (candidati: la scansione dei 22 GB di file dei logit appena scritti, la coda della corsa
CPU di T-01). Il secondo giro (`int8-2`) è stato fatto a ordine invertito, senza altro lavoro in
parallelo e con un registro del carico (`.lmbrain-lite/m16/carico.ps1`,
`<radice>\m16\carico-int8-2.tsv`): lì la patch dà 281–322 tok/s marginali contro 220–241 del
controllo, come il banco prevede. I due giri con la patch danno comunque lo stesso punteggio.

### 3.2 Compiti riusciti

| configurazione | giro | riusciti | ore di macchina | al tetto dei turni | file protetti | cause dei fallimenti |
|---|---|---:|---:|---:|---:|---|
| `notte-1` (b10809, ubatch 512) | 1 | 12/15 | 0,99 | 9/15 | 0 | tetto turni 2, tetto tempo 1 |
| G3-moro0 (b10991+moro0, ub 2048) | int8-1 | 11/15 | 1,02 | 6/15 | 0 | test falliti 2, output troncato 1, tetto turni 1 |
| G3-moro0 | int8-2 | 10/15 | 0,83 | 4/15 | 0 | test falliti 4, tetto turni 1 |
| **G3-int8** (b10991+moro1, ub 2048) | int8-1 | **10/15** | 0,87 | 4/15 | 0 | test falliti 3, tetto turni 2 |
| **G3-int8** | int8-2 | **10/15** | 0,99 | 7/15 | 0 | tetto turni 2, test falliti 2, output troncato 1 |

Compito per compito (✓ riuscito, ✗ fallito):

| compito | livello | notte-1 | moro0 1 · 2 | int8 1 · 2 |
|---|---|---|---|---|
| rs-durata-en | facile | ✓ | ✓ · ✓ | ✓ · ✓ |
| py-slug | facile | ✓ | ✓ · ✓ | ✗ · ✗ |
| ts-giorni | facile | ✗ | ✓ · ✓ | ✓ · ✓ |
| rs-durata-it | facile | ✓ | ✓ · ✓ | ✓ · ✓ |
| rs-lru | media | ✓ | ✓ · ✓ | ✓ · ✓ |
| rs-calc | difficile | ✗ | ✗ · ✗ | ✓ · ✗ |
| rs-report | media | ✓ | ✓ · ✓ | ✓ · ✓ |
| py-intervals | media | ✓ | ✗ · ✗ | ✗ · ✓ |
| py-ledger | difficile | ✓ | ✓ · ✓ | ✗ · ✗ |
| py-config-en | media | ✓ | ✓ · ✓ | ✓ · ✓ |
| py-config-it | media | ✓ | ✓ · ✓ | ✓ · ✓ |
| py-csvreport | media | ✓ | ✓ · ✗ | ✗ · ✓ |
| ts-eventi-en | media | ✓ | ✗ · ✗ | ✓ · ✗ |
| ts-eventi-it | media | ✗ | ✗ · ✗ | ✗ · ✗ |
| ts-carrello | difficile | ✓ | ✓ · ✓ | ✓ · ✓ |

**Come leggerlo.**
- **Il rumore è grande.** Fra i due giri della *stessa* configurazione cambiano 1 compito (moro0) e
  4 compiti (int8): il campionamento del G3 è a temperatura 1,0. Nove compiti su quindici danno lo
  stesso esito in tutti e quattro i giri.
- **La patch non fa perdere compiti in modo riconoscibile**: 10 e 10 contro 11 e 10. La soglia della
  regola (riferimento − 2, cioè ≥ 9) è rispettata in tutti e due i giri.
- **Le cause sono quelle già note di M-15**: test nascosti falliti e tetto dei turni. Nessun file
  protetto toccato in nessun giro. Gli errori si ripetono fra configurazioni: py-slug fallisce con
  la patch per lo stesso `slugify("Straße")` → `stra-e` che in `notte-1` aveva sbagliato il G1;
  ts-eventi-it fallisce in tutti e quattro i giri.
- **Due compiti (py-slug e py-ledger) falliscono in tutti e due i giri con la patch e riescono in
  tutti e due senza.** Le cause però sono diverse ogni volta, e sono quelle dell'harness: in
  `int8-1` un test nascosto (`slugify("Straße")`) e turni finiti a inseguire `python -c`; in
  `int8-2` il tetto dei turni con un `python` rimasto appeso 300 s e, su py-ledger, trenta turni di
  `apply_patch` con diff illeggibili. Non c'è un filo numerico: con due giri per configurazione non
  si distingue da una coincidenza, e per dirlo servirebbero più giri (pass^k).
- **`notte-1` a 12/15 sta sopra tutti**, ma è un giro solo, su un altro tag e un altro ubatch:
  con questo rumore non se ne ricava niente.
- `solo_motore_locale` è vero in tutte le 60 righe nuove: nessuna ricaduta sul cloud.

### 3.3 Perché il tempo non migliora

| configurazione | giro | richieste | token elaborati (per richiesta) | prefill | prefill effettivo | costo fisso per richiesta | costo marginale |
|---|---|---:|---:|---:|---:|---:|---:|
| G3-moro0 | int8-2 | 239 | 80.474 (337) | 713 s (24 %) | 113 tok/s | ~1,5 s | 4,2–4,6 ms/token (220–241 tok/s) |
| G3-int8 | int8-2 | 241 | 75.744 (314) | 620 s (17 %) | 122 tok/s | ~1,5 s | 3,1–3,6 ms/token (281–322 tok/s) |

Con l'agente il motore vede **tante richieste piccole**: la mediana è sotto i 60 token nuovi, e solo
una decina per giro supera i mille. Ogni richiesta paga un costo fisso di circa **1,5 secondi**
(misurato con una regressione di `prompt_ms` su `prompt_n`; a 22 token nuovi il server dichiara
1.520 ms, cioè 69 ms per token), che la patch non tocca: su 240 richieste sono circa 6 minuti per
giro, metà del tempo di prefill.

Il costo fisso non è spiegato dal log a verbosità 3. Il candidato è il salvataggio e il ripristino
dei checkpoint dello stato ricorrente dei modelli ibridi (M-15, sezione 6). **Vale anche per il
G1**: nell'avvio di `notte-1` è 1,45 s per richiesta, con un costo marginale di 2,3–2,4 ms/token.
È il numero più grande trovato qui dopo il prefill a freddo, e merita un milestone suo: vale per
ogni sessione con un agente, con o senza patch.

## 4. La regola (T-04)

Riscritta in `.lmbrain-lite/fork/README.md`, sezione «Regola di fedeltà»:
- il testo identico a temperatura 0 **non è più un requisito**: si registra e basta;
- la fedeltà si misura con la KLD contro la base, confrontata con due metri:
  - l'ubatch;
  - il backend CPU, quando il modello sta in RAM;
- soglie per modello (ognuna rispetto al più grande dei due metri):

| misura | soglia |
|---|---|
| ΔPPL | dentro 2 σ o negativa |
| KLD media | ≤ 2 × metro e ≤ 0,02 |
| KLD al 99 % | ≤ 2 × metro |
| KLD massima | ≤ 1,0, oppure in una zona dove anche il metro del backend si discosta e dove la NLL del token vero con la patch non peggiora |
| stesso primo token | ≥ metro − 1 punto |
| batteria | riusciti ≥ riferimento − 2 su 15 (stessa base e stesso ubatch), nessun file protetto, nessuna causa nuova |

**Applicata alla patch:**

| soglia | G1 (metro: CPU) | G3 (metro: ubatch) |
|---|---|---|
| ΔPPL | −0,004 ± 0,005 ✓ | −0,003 ± 0,006 ✓ |
| KLD media | 0,0102 ≤ 0,020 ✓ | 0,0092 ≤ 0,013 ✓ |
| KLD 99 % | 0,066 ≤ 0,20 ✓ | 0,113 ≤ 0,165 ✓ |
| KLD massima | 12,2: zona fragile, anche la CPU diverge, NLL della patch migliore ✓ | 3,7: token incerto, NLL della zona uguale alla base (1,018 contro 1,015). Non c'è CPU: ✓ con riserva |
| stesso primo token | 97,9 % ≥ 96,5 % ✓ | 97,1 % ≥ 96,2 % ✓ |
| batteria | non eseguita: l'ubatch del profilo non cambia (T-03) | 10/15 e 10/15 contro il riferimento moro0 allo stesso ubatch (11 e 10): ≥ 9 ✓ |

Le note di M-14 vanno aggiornate dalla sessione principale (dal worktree i file del kit non si
committano): la regola di coerenza di M-14 («stesso testo a temperatura 0») è superata da questa.

## 5. Verdetto e profili di prova (T-05)

**La patch entra.** `patch/int8-coopmat` (PR ggml-org#27952) resta nella serie `moro1`
e la serie diventa la base consigliata per il G3.

**Motivi.**
1. **Fedeltà**: tutte le soglie della regola nuova sono rispettate, su G1 e su G3 (sezione 4). La
   perplessità migliora di poco su tutti e due; il picco del G1 è spiegato ed è a favore della
   patch.
2. **Velocità**: sul G3 il prefill guadagna il 26–33 % a parità di ubatch, e il 50–57 % se si
   cambia anche l'ubatch (512 → 2048). Sul G1 il 4 %.
3. **Compiti**: la batteria non peggiora oltre il rumore (sezione 3).
4. **Costi**: decode, memoria e tempo di caricamento non cambiano; il ramo ribasa senza conflitti.

**Con due riserve, da scrivere accanto al numero:**
- **Nel lavoro con l'agente il guadagno è piccolo** (+8 % di prefill effettivo, cioè il 2 % del
  tempo di un compito), perché il prefill degli agenti è fatto di richieste piccole con un costo
  fisso che la patch non tocca. Il guadagno grande si vede sui **prompt a freddo** (−35 s su 16,8k
  token, sezione 6): prima richiesta, riaperture, compattazioni.
- **Sul G1 non conviene di per sé** (+4 %): il profilo di prova del G1 serve solo se si vuole
  passare comunque a b10991.

**Che cosa resta della regola di M-14.** La frase «una patch che cambia le risposte più di quanto il
backend cambi da solo non entra» resta, ma «di quanto il backend cambi da solo» ora si misura con
l'ubatch e con un altro backend, non con l'uguaglianza del testo.

### moro-ai

`moro-ai` **è già** b10991 + `patch/int8-coopmat`: il ramo punta a 8253abef6, lo stesso commit
della provenienza di `b10991+moro1-vulkan` (la build misurata in M-14 e qui). La serie resta
`moro1` e non serve una build nuova. Rilanciare
`build.ps1 -Tag b10991 -Patch patch/int8-coopmat -Serie 1 -Sovrascrivi` ricompilerebbe gli stessi
sorgenti sopra i binari a cui si riferiscono le misure, senza nulla da guadagnare; non è stato fatto.

### Profili di prova

Profili di prova in `<radice>\profiles`, copiati da quelli standard con la sola build (e, per il G3,
l'ubatch) cambiata e un commento in testa:

| profilo | da | build | ubatch / batch | uso |
|---|---|---|---|---|
| `qwen3-coder-next.q4_k_m.vulkan.int8` | G3 | `b10991+moro1` | 2048 / 2048 | la combinazione scelta |
| `qwen3-coder-next.q4_k_m.vulkan.moro0` | G3 | `b10991+moro0` | 2048 / 2048 | controllo della batteria; si può cancellare |
| `qwen3.6-35b-a3b.q4_k_m.vulkan.int8` | G1 | `b10991+moro1` | 4096 / 4096 (come il profilo) | +4 % di prefill; non provato con la batteria |

### Comandi per i profili standard (li esegue l'operatore)

Il profilo standard del G3 si modifica in tre righe. Dall'app (Profili → modifica) oppure da
PowerShell, dopo una copia:

```powershell
$p = "$env:AETHERA_RADICE\profiles\qwen3-coder-next.q4_k_m.vulkan.toml"
Copy-Item $p "$p.prima-di-m16"
(Get-Content $p -Raw) -replace 'build = "b10809"', 'build = "b10991+moro1"' `
    -replace 'ubatch = 512\r?\nbatch = 2048', "ubatch = 2048`nbatch = 2048" |
    Set-Content -Encoding utf8NoBOM $p   # PowerShell 7; in Windows PowerShell 5.1 usare [IO.File]::WriteAllText
```

Per il G1 basta la build:
- `build = "b10809"` → `build = "b10991+moro1"` in `qwen3.6-35b-a3b.q4_k_m.vulkan.toml`.

Il guadagno è piccolo, e il profilo standard oggi è su b10809: il cambio porta con sé anche il
passaggio a b10991, che sul G1 con MTP la batteria non ha provato. Consigliato solo dopo una prova
d'uso del profilo `.int8`.

Dopo il cambio si possono cancellare i profili di prova.

### La PR upstream

Stato letto con `gh` il 17-09 alle 13:5x (solo lettura):
- ggml-org/llama.cpp#27952 è **aperta**, non in bozza, `MERGEABLE`, revisione richiesta;
- il 15-09 0cc4m ha chiesto a jeffbolznv se restano obiezioni;
- **testa aggiornata oggi alle 08:58 UTC** (c9d34ea9f, ae6349d40): è un rebase su master dopo la
  PR #28732, che divide `ggml-vulkan.cpp` in più file. Gli shader (`mul_mmq_cm1.comp`,
  `mul_mmq_cm1_funcs.glsl`) sono identici a quelli misurati; la parte C++ è la stessa, spostata in
  parte in `ggml-vulkan-types.h`.

**Conseguenza per il prossimo tag.** Su un tag che contiene #28732 il ramo di oggi (basato su
b10991) non ribasa più in modo meccanico. Prima della build va ripreso dalla testa della PR:

```bash
git -C <radice>/src/llama.cpp fetch upstream pull/27952/head
git -C <radice>/src/llama.cpp branch -f patch/int8-coopmat FETCH_HEAD
```

Poi `build.ps1 -Tag b<nuovo> -Patch patch/int8-coopmat -Serie 1` e la misura di M-16 sul tag
nuovo (i metri cambiano con il tag).

**Quando la PR viene fusa:**
1. Controllare con `gh pr view 27952 -R ggml-org/llama.cpp --json state,mergedAt,mergeCommit`.
2. Scegliere un tag che contiene il commit di merge:
   `git -C <radice>/src/llama.cpp merge-base --is-ancestor <mergeCommit> b<nuovo>` (uscita 0).
   Se il tag si prende da ggml-org, la build della release basta; se si compila qui, con
   `build.ps1 -Tag b<nuovo> -Serie 0`.
3. Misurare «tag nuovo contro b10991+moro1» (come M-08 T-07) sui profili che usano la patch.
4. Nei profili, `build = "b10991+moro1"` diventa `build = "b<nuovo>"`.
5. Togliere il ramo: `git -C <radice>/src/llama.cpp branch -D patch/int8-coopmat`, e riportare
   `moro-ai` al tag (`build.ps1` lo rifà alla build successiva, con la serie delle patch rimaste).
6. Quando nessun profilo usa più `b10991+moro1`, togliere la sua voce da `machine.toml` e la
   cartella `<radice>\builds\llama-b10991+moro1-win-vulkan-x64`.
7. Aggiornare la tabella «Patch candidate» del README del fork.

## 6. Tempo risparmiato su un prompt a freddo (T-06)

Il prompt fisso di Claude Code è di 16.822 token (misurato il 16-09). Tempo stimato = 16.822 / prefill
misurato. Si usa il 21k del banco, il più vicino per lunghezza (13,6–17,7k token elaborati; vedi il
difetto in sezione 2), con il 7k fra parentesi.

| modello | configurazione | prefill 21k | tempo | con il 7k |
|---|---|---:|---:|---:|
| G3 | base, ubatch 512 (il profilo di oggi, a parte il tag) | 162,0 | **103,8 s** | 92,1 s |
| G3 | base, ubatch 2048 | 193,5 | 86,9 s | 76,8 s |
| G3 | patch, ubatch 512 | 207,2 | 81,2 s | 69,2 s |
| G3 | **patch, ubatch 2048** | **243,8** | **69,0 s** | **58,8 s** |
| G1 | base, ubatch 4096 | 351,7 | 47,8 s | 37,9 s |
| G1 | patch, ubatch 4096 | 364,6 | 46,1 s | 36,5 s |

**Risparmio:**
- sul G3 **35 s** per ogni prompt a freddo (−34 %; −33 s col 7k). Di questi, 17 s vengono
  dall'ubatch e 18 dalla patch;
- sul G1 1,7 s (−3,5 %).

Il prompt a freddo capita alla prima richiesta, dopo ogni compattazione e quando il client perde la
risposta precedente (M-08 T-14: sul G1 a 7k, 16,4 s invece di 6 per turno).

## 7. Cosa resta all'operatore

1. **Decidere sui profili standard.** I profili di prova sono pronti e misurati; i
   comandi per quelli standard sono nella sezione 5. Consiglio: G3 sì (build e ubatch), G1 no per
   ora.
2. **Revisione e merge del branch `m16-int8-coopmat`.** Niente push: il branch è solo locale.
3. **Note del milestone.** Dalla sessione principale: riportare nelle note di M-14 e M-16 che la
   regola di coerenza è cambiata (il testo identico a temperatura 0 non è più un requisito) e che
   `moro-ai` contiene la patch.
4. **Sorvegliare la PR #27952**: quando viene fusa, seguire i sette passi della sezione 5. La testa
   della PR è già stata ribasata su un master che divide `ggml-vulkan.cpp`: il ramo locale va
   ripreso dalla PR prima del prossimo tag.
5. **Il costo fisso di 1,5 s per richiesta** (sezione 3.3) merita un milestone suo: vale per il G1 e
   per il G3, in ogni sessione con un agente.
6. **Il disturbo del primo giro della batteria** (sezione 3.1) non è stato identificato. Se ricapita,
   il registro del carico (`carico.ps1`) ora c'è.
7. **File sulla macchina:** i file dei logit (22 GiB) sono stati cancellati; restano le uscite
   testuali in `<radice>\m16\`, le righe dei banchi (`T-02a.jsonl`, `T-02b.jsonl`), i risultati
   delle batterie in `<radice>\m15\risultati\int8-1` e `int8-2`, e i tre profili di prova in
   `<radice>\profiles` (quello `.moro0` si può cancellare).

## Sigle

| sigla | che cosa |
|---|---|
| **G1** | Qwen3.6-35B-A3B Q4_K_M, profilo `qwen3.6-35b-a3b.q4_k_m.vulkan` (MTP 3, ubatch 4096) |
| **G3** | Qwen3-Coder-Next Q4_K_M, profilo `qwen3-coder-next.q4_k_m.vulkan` (ubatch 512, batch 2048) |
| **moro0** | `b10991+moro0`: il tag b10991 compilato qui, senza patch (la «base») |
| **moro1** | `b10991+moro1`: b10991 + PR #27952 (int8 coopmat), commit 8253abef6 (la «patch») |
| **KLD** | divergenza di Kullback-Leibler della distribuzione dei token dalla base, in nat per token |
| **metro** | quanto la base cambia da sola: con un altro ubatch, o calcolata da un altro backend |
