# M-17 T-06 — evitare la copia dei checkpoint: tre vie, valutate e non applicate

> 18-09-2026, notte. Valutazione scritta su richiesta del milestone (T-06): **nessuna patch è stata
> applicata**. Sorgente letto: `<radice>\src\llama.cpp`, HEAD `930e2fa59` = tag b10991. La regola del
> fork (`fork/README.md`, regola 1) dice che le patch sono piccole, ribasabili sul tag e destinate a
> sparire: una patch che riscrive un backend non entra.

**Il fatto da cui si parte** (misurato, M-17 T-03): su G3 il 94 % del costo fisso per richiesta è la
creazione di tre context checkpoint, 75,4 MiB l'uno, copiati dispositivo→host **un tensore per
volta**. 130 MB/s su una macchina che ne fa 75.000. Il collo non è la banda, è il submit +
`waitForFences` + pulizia dei command pool che il backend Vulkan fa per ogni chiamata
(`ggml-vulkan.cpp:8943-8994`), moltiplicato per una settantina di tensori.

## Dove sta il codice

| cosa | dove |
|---|---|
| flag `LLAMA_STATE_SEQ_FLAGS_ON_DEVICE` | `include/llama.h:916` (valore 2, PR upstream #22679) |
| percorso «on device» | `llama-context.cpp:3082-3138` → `llama_io_write_device`/`read_device` |
| percorso host (quello usato oggi) | `llama_io_write_host::~`, `llama-context.cpp:2585-2589`, con il TODO «add backend support to batch tensor_get» alla riga 2586 |
| il server crea i checkpoint | `tools/server/server-context.cpp:2309-2372`, solo con `PARTIAL_ONLY` (righe 2363-2364) |
| il dato del checkpoint | `common_prompt_checkpoint`, `common/common.h:1165-1179`: tre `std::vector<uint8_t>`, cioè **RAM host** |
| limite | `--ctx-checkpoints`, default **32** (`common/common.h:629`, `common/arg.cpp:1697`); si evince il più vecchio quando la lista è piena, **nessun limite in byte** |

Il flag on-device oggi è usato solo nei test (`tests/test-save-load-state.cpp:323-372`). Il server
non lo passa mai. `llama-memory-recurrent.cpp:824` **aborta** se lo stato ha più di un intervallo di
celle: non è un ripiego silenzioso, è un `GGML_ABORT`.

## Le tre vie

### 1. Tenere i checkpoint sul dispositivo (ON_DEVICE)

Sono poche righe: un `| LLAMA_STATE_SEQ_FLAGS_ON_DEVICE` nelle chiamate di
`update_tgt`/`update_dft` e nelle `load_*` corrispondenti. Il percorso esiste già e accumula le
scritture per farne **una sola** allocazione e copie dispositivo→dispositivo.

**Ma sposta la spesa dalla RAM alla VGM.** Oggi quei 32 × 75 MiB (2,4 GiB) stanno in RAM host, dove
ne abbiamo in abbondanza; on-device diventerebbero 2,4 GiB di VGM, che è la risorsa scarsa: il G3
sta già al limite a VGM 48 e spilla in memoria condivisa. Su questa macchina **non è un guadagno
gratuito**, è uno scambio da misurare.

E rompe tre cose se applicato al checkpoint persistente: la prompt cache su RAM (`--cache-ram`), il
salvataggio dello slot su disco e `/slots` vogliono tutti byte host.

**Upstream ci sta già lavorando.** PR ggml-org/llama.cpp **#28118**, aperta in bozza: «server: keep
speculative recurrent-state checkpoints on-device». Applica il flag **solo ai checkpoint speculativi
transitori**, non a quello storico, proprio per non rompere cache e disco. Le misure dell'autore, su
Strix Halo con Vulkan, dicono 600 ms su 825 di ogni giro spesi nel viaggio verso l'host: lo stesso
fenomeno che abbiamo misurato noi. **Da seguire e ribasare quando esce dalla bozza**, non da
riscrivere.

### 2. Raggruppare le copie

È quello che il TODO propone, ed è la via che darebbe di più: un submit invece di settanta. Ma in
ggml non esiste un «batch get» verso l'host: andrebbe aggiunta un'API di backend e implementata in
`ggml-vulkan.cpp`. **Fuori dalla regola del fork**: si tocca il backend, non lo stato. Da scartare
per noi; semmai da proporre upstream come issue, con i nostri numeri, che sono buoni.

### 3. Creare meno checkpoint

Costa **zero righe di codice**: sono leve del server già esistenti. Il guadagno è lineare — tre
checkpoint per richiesta diventano uno, o nessuno. Il prezzo è che dopo una divergenza dentro il
testo generato non c'è niente da cui ripartire e si ricalcola tutto il prompt.

**Per noi il prezzo potrebbe essere nullo**: un harness che scrive solo in coda non diverge mai. La
lettura del codice di Nonio (`reports/nonio-harness-2026-09.md`) dice che la coda è davvero in sola
aggiunta; restano tre punti che la rompono (il ragionamento non rimandato, il turno troncato
sostituito, le chiamate malformate tolte), e sono correggibili.

## Raccomandazione

1. **Misurare la via 3** — è M-17 T-05, già in coda stanotte: lo scenario con `--ctx-checkpoints`
   a 0, 1 e 32, e con una divergenza provocata per sapere quanto costa quando capita. Se conferma,
   è una leva di profilo, non una patch.
2. **Seguire la PR #28118** e ribasarla nel fork quando esce dalla bozza, misurando anche quanto
   costa in VGM: è la via 1 fatta da chi ha già valutato i rischi.
3. **Non fare la via 2.** Semmai aprire un'issue upstream con i nostri numeri (130 MB/s, tre
   checkpoint per richiesta, 94 % del costo fisso su un modello ibrido con Vulkan UMA): nessuno
   sembra lavorare sul batching lato backend.

Da decidere all'operatore: se aprire quell'issue a nome del progetto, e se le correzioni all'harness
sul riuso (che rendono sicura la via 3) hanno la precedenza sulle altre misure.
