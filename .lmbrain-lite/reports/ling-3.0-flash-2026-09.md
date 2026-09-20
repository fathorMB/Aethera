# Ling-3.0-flash contro il G1, e il ragionamento acceso sul Q8

Prova una tantum, non un milestone. I dati grezzi stanno in `<radice>/m19/` e nelle
sessioni `m19-*` di `<radice>/m15/risultati/`: quella sigla e' precedente all'assegnazione
di M-19 alla speculazione, ed e' rimasta solo nei nomi delle cartelle.

Notte del 20 settembre 2026, 02:36-08:40. Tutto su b10809 Vulkan, driver 32.0.31041.1004,
VGM 64, ctx 32768, batteria di coding con Nonio, un solo motore acceso da Aethera.

## La domanda

Ling-3.0-flash (124B totali, 5,1B attivi, MoE 8/512, attenzione ibrida KDA+MLA 5:1) sembrava
la forma giusta per questa macchina: dall'intestazione GGUF legge **3,393 GB per token**, appena
il 3,6% in più dei 3,276 del G1 Q8, portando 3,5 volte i parametri totali. Q3_K_M perche' il
Q4_K_M e' 74,85 GB e non entra in VGM 64.

Insieme, la misura che mancava: il **ragionamento acceso sul profilo principale Q8**. C'era solo
sul Q4 (M-18 T-11) e su quattro compiti.

## Il verdetto

| giro | risolti | minuti | **compiti/ora** | riuso | prefill | decode tok/s | chiamate fallite |
|---|---:|---:|---:|---:|---:|---:|---:|
| **Qwen Q8, spento** | 14/16 | 24,1 | **34,9** | 91,5% | 4,2 min | 25,3 | 1/197 (0,5%) |
| Qwen Q8, acceso, rispedito | 14/16 | 36,1 | **23,2** | 81,3% | 5,1 min | 24,6 | 5/166 (3,0%) |
| Qwen Q8, acceso, non rispedito | 14/16 | 54,7 | 15,4 [1] | 80,1% | 6,0 min | 24,7 | 5/174 (2,9%) |
| **Ling Q3_K_M, acceso, rispedito** | **12/15** | **157,4** | **4,6** | **20,4%** | **92,2 min** | 15,7 | 20/148 (13,5%) |

[1] Un turno troncato: per la regola dichiarata prima della notte, quel numero non vale come
misura. La direzione (rispedire conviene) resta, perche' coincide con il Q4.

### Ling: no, e non per poco

**4,6 contro 34,9 compiti per ora.** Su 157 minuti, **92 in prefill** contro i 4,2 del Qwen;
384.220 token rielaborati contro 95.594.

## Le previsioni, verificate una per una

Fatte **prima** di scaricare i pesi, dai soli 32 MB di intestazione via richieste HTTP con range.

| previsione | misura | esito |
|---|---|---|
| byte per token 3,393 GB | 3,393 GB (sui file interi, chiudono a 6,5 MB su 59) | **esatta** |
| VRAM dedicata ~55,8 GiB | 55,52 GiB | **esatta** |
| ci sta in VGM 64 | condivisa 0,50 GiB | **esatta** |
| decode 14-18 tok/s | 16,32 (llama-bench), 15,7 (batteria) | **esatta** |
| banda utile ~88% del G1 (512 esperti) | 55,4 contro 63,3 GB/s = 87,5% | **esatta** |
| prefill ~1,7x piu' lento (5,1B attivi contro 3B) | **3,3x** (121,7 contro 399,2 tok/s a pp512) | **sbagliata del doppio** |

Il modello della banda regge dall'intestazione al banco alla batteria. Il conto dei parametri
attivi **non** predice il prefill: manca un pezzo, e i candidati sono il percorso KDA su Vulkan
o il routing a 512 esperti. Dichiarato come non saputo.

## Il riuso del prefisso: la causa, e un'ipotesi scartata

Ling riusa il **20,4%** del prefisso, il Qwen il 91,5%. Non e' il client a mandare un prefisso
instabile: confrontando i messaggi turno per turno nella traccia di Nonio, il prefisso e'
**stabile** («turno N -> N+1: prefisso intatto» per tutti i turni). Non e' compattazione (zero).

Non e' nemmeno il ragionamento: **rifatta la prova coi quattro compiti e `enable_thinking`
spento, il riuso resta al 25,7%** (contro il 21,4% acceso). Senza un solo token di pensiero il
problema c'e' lo stesso. Di passaggio: spegnere il ragionamento **peggiora** Ling, 3/4 invece di
4/4 e il 23% di chiamate agli strumenti fallite contro lo 0%.

### L'ipotesi che avevo formulato, e perche' e' sbagliata

Avevo scritto che la KDA, essendo attenzione lineare **ricorrente**, non puo' riusare un
prefisso a pezzi: si puo' continuare ma non riavvolgere. Il comportamento a tutto-o-niente di
Ling (turni al 92,7% alternati a turni allo 0,0%, mai valori intermedi) sembrava l'impronta
digitale di quel meccanismo.

**La prova diretta la smentisce.** Parlando a `llama-server` con `/completion` grezzo — niente
template di chat, niente strumenti, niente client — misurando il riuso da `timings.cache_n`:

| prova | Ling (KDA ibrida) | Qwen Q8 (attenzione classica) |
|---|---:|---:|
| appendice pura, 5 richieste | **99,7%** | **99,7%** |
| una parola cambiata a meta' del prompt | **0,0%** | **0,0%** |

Il Qwen si comporta **identicamente**. Se la causa fosse lo stato ricorrente, il Qwen avrebbe
dovuto riusare ~50% nella seconda prova; non lo fa. **L'architettura e' scagionata.**

### Quello che la prova ha trovato davvero

Un fatto sull'motore che non era scritto da nessuna parte e che vale per **tutti** i modelli:

> `llama-server` b10809, con questi profili, riusa il prefisso **solo quando il prompt nuovo e'
> un'estensione esatta di quello in cache**. Alla prima divergenza, ovunque cada, il riuso non
> e' parziale: e' **zero**, e si rielabora tutto da capo.

Quindi il 20,4% di Ling non dice «Ling ha una cache rotta»: dice che **le conversazioni di
Ling divergono molto piu' spesso di quelle del Qwen**.

### Due cause proposte, due cause smentite

**Prima ipotesi: lo stato ricorrente della KDA.** Smentita dalla prova su `/completion` grezzo
(sopra): il Qwen si comporta identicamente, quindi non e' l'architettura.

**Seconda ipotesi: il formato degli argomenti delle chiamate.** `openai.rs:88` manda gli
argomenti come stringa JSON per ogni famiglia, e il template Bailing V3 li itera con `.items()`,
che su una stringa e' un errore. Reso con jinja2 puro, la differenza c'e' davvero.

**Smentita anche questa, dal motore.** `llama-server` b10809 normalizza prima di rendere: le
caps del template (`/props` -> `supports_object_arguments: true`) fanno convertire la stringa in
oggetto. Rendendo la stessa conversazione con `/apply-template` nelle due forme:

| argomenti mandati come | prompt reso |
|---|---|
| stringa | `<tool_call>read_file<arg_key>path</arg_key><arg_value>src/lib.rs</arg_value>` |
| oggetto | **identico, carattere per carattere** |

La prova con jinja2 misurava il template **senza** la pipeline di llama.cpp. Quindi la frase
«Ling ha corso con lo storico delle chiamate mutilato» era **falsa**, ed e' stata tolta.

### Dove sta davvero, per quanto ne sappiamo

Rendendo con `/apply-template` i prompt veri di un compito di Ling, turno per turno:

```
0->1  ESTENSIONE ESATTA      1->2  ESTENSIONE ESATTA      2->3  ESTENSIONE ESATTA
3->4  ESTENSIONE ESATTA      4->5  ESTENSIONE ESATTA
```

Ogni prompt estende esattamente il precedente. E su quello stesso compito il motore ha riusato
**zero token su sei turni**, rielaborandone 35.548 e spendendo 501 s di prefill.

Il prompt non c'entra. E non c'entra nemmeno Nonio: la perdita si **riproduce con `curl` e basta**,
in una conversazione multi-turno con strumenti fatta a mano contro l'endpoint `/v1/chat/completions`:

```
turno 1: rielab  249  cache 0  riuso 0,0%
turno 2: rielab  589  cache 0  riuso 0,0%
turno 3: rielab  995  cache 0  riuso 0,0%
turno 4: rielab 1368  cache 0  riuso 0,0%
```

Mentre una conversazione a **due** turni con strumenti riusa l'87,2%, in tutte e quattro le
varianti provate (con e senza `enable_thinking`, in streaming e no). Qualcosa fra il secondo e il
terzo turno fa perdere la cache, e **non l'ho isolato**. Sta nel percorso `bailingmoe3` di
llama.cpp o nel template Bailing, non nel client.

**Non propongo una terza ipotesi.** Le prime due sembravano solide e sono cadute tutte e due
davanti alla misura; questa riga resta aperta finche' qualcuno non la misura.

### Perche' non cambia il verdetto

I numeri della **batteria** su Ling sono piu' severi del dovuto: misurano Ling con la cache che
si perde, non Ling. I numeri del **banco** no — `llama-bench` non passa da nessun client:

| | Ling | Qwen Q8 | rapporto |
|---|---:|---:|---:|
| prefill 512 | 121,67 tok/s | 399,25 | **0,30x** |
| decode 128 | 16,32 tok/s | 19,33 | **0,84x** |

**Il verdetto poggia su questi.** E anche concedendo tutto il resto: stima lineare sul prefill,
con riuso al 90% Ling passerebbe da 5,5 a **13,7 compiti/ora** sui quattro compiti della prova
corta, contro i 30,0 del Qwen sugli stessi. Un modello che prefilla a un terzo, su una macchina
dove il prefill e' gia' il vincolo, non diventa competitivo riparando una cache.

### Una conseguenza per il resto del lavoro

Che il riuso sia tutto-o-niente alza il valore di **M-18 T-14** molto oltre quello che sembrava:
non si tratta di recuperare 11 punti di media, si tratta che **ogni** divergenza costa un
prefill intero. Su un contesto lungo, dove un prefill a freddo a 200k costa ~28 minuti, e' la
differenza fra usabile e inusabile.

## Il ragionamento acceso sul Q8: non conviene

**34,9 spento contro 23,2 acceso.** Un terzo della produttivita', e **14/16 risolti in tutti e
due i casi**: 15.693 token di pensiero pagati per lo stesso risultato. Il profilo principale
resta giusto com'e', con `thinking = false`.

Rispedire il ragionamento vale **+51%** (23,2 contro 15,4), lo stesso verso misurato sul Q4
(30,0 contro 17,9). Se un giorno lo si accende, si rispedisce.

## Limiti dichiarati

- Il runner **impone `family.kind = "qwen"`** a ogni modello: Nonio conosce solo `qwen`,
  `gptoss` e `generic`. Ling e' stato guidato con convenzioni non sue, e il 13,5% di chiamate
  fallite (contro lo 0,5-3,0% del Qwen) probabilmente viene da li'. Quindi questa notte non ha
  misurato «Ling», ha misurato **«Ling come Nonio sa guidarlo oggi»**. E' la misura giusta per
  decidere se adottarlo adesso; non e' un giudizio sul modello.
- Il GGUF di bartowski **non ha i tensori MTP** (`nextn_predict_layers = 1` nei metadati,
  `mtp_types` vuoto): Ling ha girato senza speculazione, il G1 con MTP. Sui tok/s il confronto
  e' a sfavore di Ling; su compiti/ora ogni modello corre come si puo' davvero far correre.
- Un giro per configurazione, non due: la varianza non e' misurata. Con un divario di 7,5 volte
  non cambia la conclusione; sul confronto acceso/spento del Qwen (34,9 contro 23,2) un secondo
  giro servirebbe.
- La scala del contesto (32k/131k/262k) e' stata **interrotta**: il prefill a 262k ripetuto tre
  volte costava ore e avrebbe mangiato la batteria. Resta da fare, e adesso interessa meno.

## Cosa resta

1. **Isolare che cosa fa perdere la cache a Ling fra il secondo e il terzo turno**, con la
   riproduzione a `curl` gia' scritta: e' il pezzo che manca, ed e' nel motore, non nel client.
2. **M-18 T-14 va riscritto**: il sospetto su `serde_json` e' smentito (`preserve_order` c'e'
   gia'), e l'attribuzione «from system» che lo alimentava e' circolare. Quello che resta di
   vero e' piu' grande: il riuso e' **tutto-o-niente**, quindi ogni divergenza costa un prefill
   intero. Trovare le divergenze residue del Qwen resta il lavoro, con un'altra pista.
3. L'ipotesi sull'attenzione ibrida lineare e' stata **provata e scartata**: non c'e' nessuna
   controindicazione architetturale per Kimi K3, Ling o quel filone.
2. Il milestone sulla speculazione (materiale in `m08/speculazione-materiale.md`): con l'82,4%
   di accettazione gia' misurato sul lavoro vero, la domanda e' perche' compra solo il +36%.
