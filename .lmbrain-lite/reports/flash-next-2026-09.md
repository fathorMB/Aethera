# Qwen3.8-Flash-Next intero sulla Minisforum: che cosa dicono le misure

> **M-10, 16/18-09-2026. Chiuso con un no il 18-09 per decisione dell'operatore.** Le misure di
> velocità, di memoria, di contesto lungo e di riuso ci sono tutte (T-04, T-07, T-08, T-09). La
> batteria di coding di FN (T-10) non si fa: il verdetto sulla *qualità* di FN come agente resta
> «sconosciuto», non zero, ma quello sulla *velocità* basta (8-9 tok/s, un compito in 407 s contro 41
> del G1). Con la direzione nuova (massimo rendimento) FN esce dai candidati; i pesi restano sul disco. Sigle: FN = Flash-Next intero, FC = Flash-Coder, G1 = Qwen3.6-35B-A3B,
> G3 = Qwen3-Coder-Next. Motore `b10991` Vulkan, KV f16, ubatch 2048, un solo slot.

**In breve.**
- **FN gira, ma lento: 8–9 token al secondo di decode** in ogni configurazione provata, contro i
  15–18 attesi dal ragionamento sulla banda. La soglia fissata per andare avanti (≥ 12 tok/s e
  prefill caldo ≥ 100) non è raggiunta sul decode: **T-11 (MTP) e T-12 (VGM 72, IQ4_XS) non
  scattano.**
- **A VGM 48 ci sta solo con `--load-mode none`**, con 6,7 GiB spillati in memoria condivisa e la
  tabella n-gram in RAM privata: restano 2,9 GiB liberi il 16-09 e, dal 17-09, non ci sta più
  (il file di paging cresce di 4,7 GB e il sorvegliante ferma il motore).
- **A VGM 64 con `mmap` + `--lazy-mode auto` va meglio** (prefill +39 %, decode +17 %, niente
  spill), ma la RAM disponibile resta fra 0,1 e 0,5 GiB per tutta la sessione: la macchina non può
  fare altro.
- **La lettura pigra della tabella richiede `mmap`.** Con `none` il flag `--lazy-mode auto` non fa
  niente: il processo legge tutti i 76,3 GiB al caricamento e la tabella finisce in memoria privata.
- **Il riuso del prefisso è quello dei modelli ibridi**: buono per una chat che cresce in coda,
  nullo dopo una modifica a metà prompt, e nessun flag dei checkpoint lo recupera (sezione 4).
- **L'unico dato di lavoro vero** è un compito della batteria del 17-09 sera: FN lo risolve in
  407,5 s e 12 turni; G1 lo chiude in 40,9 s.

## 1. Le misure, una tabella

Prompt congelati di M-08 (`codice-7k`, `riassunto-21k`), 5 giri dopo uno di riscaldamento, macchina
ferma, sorvegliante della RAM attivo. Prefill e decode in token al secondo.

| configurazione | VGM | caricamento | pronto in | GPU dedicata + condivisa | 7k prefill | 7k decode | 21k prefill | 21k decode |
|---|---:|---|---:|---|---:|---:|---:|---:|
| FN IQ3_XXS | 48 | `none` | 42,8 s | 46,64 + 6,68 GiB | 98,6 | 7,97 | 74,2 | 6,85 |
| FN IQ3_XXS | 48 | `mmap` + lazy | non arriva a pronto (RAM libera 1,2 GiB a 51 s: doppia copia) | | | | | |
| FN IQ3_XXS | 64 | `mmap` + lazy | 115,6 s | 52,41 + 0,25 GiB | 137,1 (freddo 107,5) | 9,33 | 101,7 | 8,02 |
| FN Q3_K_XL | 64 | `mmap` + lazy | 86 s | 60,3 GiB | 122,0 | 8,73 | 90,9 | 7,57 |
| *per confronto:* FC Q4_K_M | 48 | `none` | 20 s | 28,4 + 0,59 GiB | 181,0 | 10,45 | 161,7 | 9,92 |
| *per confronto:* G1 (M-08 T-02) | 48 | | | | | 21,8 | | |
| *per confronto:* G3 (M-08) | 48 | | | | | 17,75 | | |

Il primo tentativo a VGM 64 (T-07, con la soglia di 2 GiB sulla RAM disponibile) non è arrivato a
pronto per nessuno dei due quant: durante il caricamento le pagine mappate del corpo restano nel
working set del processo (fino a 23,8 GiB) e la RAM disponibile scende sotto 1,5 GiB. Tolta la
soglia (T-07b, scelta dell'operatore), Windows regge senza paginare in modo serio: file di paging
da 204 a 733 MB.

**Perché il decode è la metà dell'atteso.** Non è la memoria: FC, che è lo stesso tipo di modello
(`qwen4exp`) con 28 GiB tutti in VGM e senza tabella, fa 10,45 tok/s, la metà di G1 che ne occupa
22. Il sospetto sono i kernel `qwen4exp` del backend Vulkan, non la banda. Non è stato verificato
con un profilo per operazione: è un'ipotesi, e va detta tale.

## 2. Contesto lungo (T-08)

FN IQ3_XXS, VGM 64, `mmap` + lazy, ctx 65536, 2 giri: pronto in 107,5 s, 54,6 GiB dedicati.

| token nel prompt | prefill | decode | tempo del prefill |
|---:|---:|---:|---:|
| 7.101 | 137,1 | 9,33 | 52 s |
| 29.289 | 85,8 | 7,47 | 5 min 41 s |
| 57.873 | 62,8 | 6,29 | 15 min 22 s |

Il calo con il contesto di Gated DeltaNet su Vulkan (issue #28734) c'è ed è forte sul prefill:
a 58k un prompt a freddo costa un quarto d'ora. Il file di paging è cresciuto di 1,7 GiB.

## 3. La tabella n-gram e la memoria (T-04, T-07)

- `--lazy-mode on` «requires mmap», e `auto` vale «on, solo per tensori > 4 GiB» (help di b10991).
- Con `none`: tabella in memoria privata (working set privato fino a 34,9 GiB, mappato ~0), zero
  byte letti dal disco dopo il caricamento. Il costo della tabella su disco a VGM 48 non è quindi
  misurabile.
- Con `mmap` a VGM 64: il prefill a freddo (tabella ancora sul disco) è 107,5 contro 137,1 a caldo,
  cioè **−22 % al primo prompt**, poi la page cache la tiene. Il contatore dei byte letti del
  processo non vede le letture da file mappato: le pagine mappate si leggono dal working set.
- `-ot "per_layer_token_embd=CPU"` resta obbligatorio: Vulkan non ha l'operazione.
- Un'entrata della prompt cache di FN a 7k pesa 530–640 MiB: con FN `--cache-ram` va a 0.

**T-05 (trappola Windows, issue #28355) non è stato fatto**, e a VGM 48 non si può fare: il
confronto chiede `lazy-mode auto` contro `off`, e la lettura pigra esiste solo con `mmap`, che a
VGM 48 non arriva a pronto. A VGM 64 il confronto non ha un lato «off»: la tabella in RAM (26,8 GiB)
non sta nei 23 GiB che restano. Resta da decidere se lasciarlo cadere.

## 4. Riuso del prefisso (T-09, con M-15 T-06)

Quattro turni su `/completion`, prompt da 7,1k, temperatura 0:

| turno | che cosa cambia | token riusati |
|---|---|---:|
| 1 | coda diversa, prefisso identico | 5.053 su 7.101 (71 %) |
| 2 | una riga cambiata a metà | 0 |
| 3 | uguale al turno 2 | 0 |

- Il 71 % è spiegato dal codice: il motore riparte dal checkpoint a `n − 4 − ubatch`, cioè
  7.101 − 2.048 = 5.053.
- I checkpoint a metà prompt nascono solo all'inizio di un messaggio utente (PR #22929): su
  `/completion` dopo una divergenza non c'è niente da riusare.
- Provato il 17-09 sera a VGM 48: `--ctx-checkpoints 16 --checkpoint-min-step 0 --cache-ram 0` dà
  lo stesso riuso (5.048 su 7.096, poi 0) e un prefill un po' più lento (113,4 contro 117,6).
  **Esito negativo, task chiuso.**
- Per una chat che cresce in coda il riuso dei modelli ibridi resta buono (G3 94 % con Nonio).

## 5. Verdetto per quant

| quant | sta in macchina | velocità | verdetto |
|---|---|---|---|
| UD-IQ3_XXS (82 GB) | VGM 64 con `mmap`; a VGM 48 non più | 9,3 / 137 | l'unico praticabile; lento |
| UD-Q3_K_XL (90 GB) | solo VGM 64, 60,3 GiB dedicati | 8,7 / 122 | più lento e più stretto di IQ3_XXS: nessun motivo per sceglierlo senza una prova di qualità |
| UD-IQ4_XS | servirebbe VGM 72 | non misurato | non provato: la condizione di T-12 (≥ 12 tok/s) non è soddisfatta |

**Come modello quotidiano FN non è candidato**: a parità di compito G1 è dieci volte più veloce
sull'unico confronto disponibile, e FN chiede di spostare la VGM a 64 e di lasciare la macchina
senza RAM libera. Può avere senso solo come modello «da chiamare per un compito difficile», e per
dirlo serve la batteria (T-10): se a VGM 64 FN non risolve compiti che G1 e G3 sbagliano, il
milestone si chiude con un no.

**Ricetta, se lo si usa** (VGM 64, profilo `qwen3.8-flash-next.iq3_xxs.vulkan`):

```
--n-gpu-layers 999 -ot "per_layer_token_embd=CPU" --load-mode mmap --lazy-mode auto
--ctx-size 32768 --parallel 1 --flash-attn on --ubatch-size 2048 --batch-size 2048
--cache-ram 0 --jinja
```

Niente altro in esecuzione, sorvegliante sul file di paging (non sulla RAM disponibile, che sta a
zero per costruzione), e mettere in conto 2 minuti di caricamento.

## 6. Che cosa resta aperto

| che cosa | di chi | note |
|---|---|---|
| T-10, batteria di FN | operatore (VGM 64 e riavvio), poi agente | `python .lmbrain-lite/m15/notte.py --sessione notte-1` riprende da solo; circa 2 ore e mezza |
| T-03, condizioni fisse | agente | di fatto applicate in tutte le misure (lock, prompt congelati, 5 giri, campionamento ogni 2 s): da chiudere con il via dell'operatore |
| T-05, trappola Windows | operatore decide | non fattibile come scritto (sezione 3): proposta, lasciarlo cadere |
| T-11 MTP, T-12 VGM 72 | — | condizione non soddisfatta: non si fanno |

## 7. Correzioni per lo studio e campi per Aethera

**`design/studio-motore-2026-09`, pagine Qwen3.8 e SSD:**
- la stima «15–18 tok/s senza MTP» va sostituita con il misurato: 8–9,3;
- `--lazy-mode` richiede `mmap`: la ricetta `none` + lazy non esiste;
- a VGM 48 `mmap` non arriva a pronto (doppia copia, come M-08 T-09); a VGM 64 regge solo senza
  soglia sulla RAM disponibile;
- il prefill cala del 54 % fra 7k e 58k token.

**Campi che servono ad Aethera:**
- `lazy_mode`, `tensor_overrides`, `cache_ram`, `ctx_checkpoints` e `checkpoint_min_step` ci sono
  già nello schema del profilo (M-09 T-06) e il profilo di FN li usa: niente da aggiungere;
- nel manifest dell'avvio: la VGM al momento dell'avvio e le pagine mappate del processo, tenute
  separate dalla memoria privata, perché con `mmap` la «RAM libera» da sola inganna;
- un avviso quando `lazy_mode` è acceso con `load_mode = "none"`: il flag non ha effetto;
- un avviso quando `cache_ram` non è 0 su un modello ibrido con poca RAM libera.

Dati grezzi: `<radice>\m10\T-04*.jsonl`, `T-07b`, `T-07c`, `T-08`, `T-09` con i rispettivi
`*-memoria.csv` e `*.meta.json`; `<radice>\m15\T-06.jsonl` per la prova dei checkpoint.
