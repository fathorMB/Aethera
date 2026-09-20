# Motori di servizio

Da M-20 Aethera può tenere accesi, accanto al motore principale, uno o più `llama-server`
piccoli: i **motori di servizio**. Questa pagina dice che cosa sono, perché esistono e dove sta
il confine con il motore principale.

## Perché esistono

Non per andare più veloce. Su questa macchina un modello più piccolo **non** è più veloce del
grosso: M-08 T-02 ha misurato che un denso da 8B fa 16,01 tok/s di decode e il MoE da 35B ne fa
24,30, perché il decode è limitato dalla banda e il MoE legge meno byte per token.

Esistono per due ragioni diverse, e conviene tenerle separate.

1. **Endpoint che il modello grosso non può dare.** Un `llama-server` serve un modello solo. Chi
   ha bisogno di embedding e rerank — GalaxyCenter, per esempio, che li pretende nel suo contratto
   e dichiara che «i server li gestisce l'operatore» — con un motore solo non è servito. Fino alla
   0.2.0 quell'operatore, cioè Aethera, ne accendeva uno.
2. **Non toccare la cache del prefisso del motore principale.** Il riuso è **tutto-o-niente**
   (misura del 20-09) e il profilo unico ha `n_parallel = 1`: infilare un'altra conversazione nello
   stesso slot fa ripartire da zero il client che sta lavorando, e a 262144 un prefill a freddo
   arriva a ~28 minuti. Un processo separato è la protezione, non l'ottimizzazione.

Lo studio che ha portato qui, con il no misurato alla NPU e al profilo principale più leggero, è
`reports/npu-e-modelli-piccoli-2026-09-20.md`.

## Che cosa li distingue dal motore principale

| | motore principale | motore di servizio |
|---|---|---|
| quanti | uno alla volta | più di uno, ognuno sulla sua porta |
| manifest, telemetria, storico | sì, sono il punto | **no** |
| stato «in uso» | sì, e blocca arresto e riavvio | **no**, e non blocca niente |
| leve | tutte, esplicite | **meno**, non una in più |
| si confronta con ieri | sì, in Benchmark | no: o risponde o no |

Il codice segue la stessa divisione: `src-tauri/src/service.rs` è lo schema (un tipo suo, non una
variante di `Profile`), `src-tauri/src/services.rs` è il ciclo di vita. Un profilo di servizio sta
in `profiles/` come gli altri e si riconosce da `role = "service"`; `read_profiles` lo salta, e
`services::list_profiles` guarda solo quelli.

Le leve che un servizio **non ha** — speculazione, checkpoint e `cache_reuse`, budget di contesto
del client, `slot_save`, campionamento consigliato — non sono spente: non esistono nel tipo, e
`deny_unknown_fields` le rifiuta al caricamento dicendo quale campo non appartiene lì.

Dove un servizio accetta il default del motore (ubatch, batch, flash attention, tipo di cache) è
perché su un modello da 0,6 GB quelle leve non spostano niente di misurabile. È una scelta
dichiarata nel commento del modulo, non una dimenticanza: per il motore principale la regola resta
quella di sempre, niente lasciato al default.

## La memoria

Si misura, non si stima, e **non** con `llama-server --list-devices`: quella riga riporta il
budget dell'heap Vulkan, che su questa macchina vale `81740 MiB, 77653 MiB free` identico al byte a
motore spento, a motore carico e a motore carico più compagno. Il numero vero viene dai contatori
di Windows, gli stessi che Aethera usa già:

- `\GPU Adapter Memory(*)\Dedicated Usage` per il totale;
- `\GPU Process Memory(pid_N_*)\Dedicated Usage` per un singolo processo, ed è così che si legge
  quanto occupa ogni servizio.

Misure del 20-09 sera su questa macchina, VGM 64,00 GiB:

| | dedicata in uso | libera |
|---|---|---|
| profilo unico a 262144, da solo | 46,76 GiB | 17,24 |
| più un compagno Qwen3-8B Q4_K_M a ctx 8192 | 52,52 GiB | 11,48 |

Il compagno da 8B prende 5,76 GiB ed è un limite superiore generoso: embedding 0,6B e reranker
0,6B insieme stanno attorno ai 2 GiB.

## La precedenza al coding

Decisa dall'operatore il 20-09: i ruoli non di coding possono girare anche di giorno, ma il coding
ha la precedenza. **Il meccanismo non sta in Aethera**, e non deve starci: Aethera dice com'è
messa, chi usa i servizi decide se fermarsi. Sospendere d'autorità un processo che sta rispondendo
farebbe scadere le richieste in volo e metterebbe qui una politica di un'altra applicazione.

Quello che Aethera espone già su `GET /status` (`127.0.0.1:8090`): `in_use` con le sue `reasons` —
slot attivo, richiesta negli ultimi 30 s, lock dichiarato, protezione manuale — e ora anche
l'elenco dei servizi accesi, **accanto** allo stato e non dentro `usage`.

**`in_use` da solo non basta, ed è misurato.** Il 20-09, con una sessione di coding aperta ma ferma
da 21 minuti, `/status` rispondeva `in_use: false` e `locks: []`: la finestra è di 30 secondi, e
fra un turno e l'altro il motore sembra libero. Chi lavora deve prendere un **lock**
(`POST /lock`, con TTL), che Aethera ha già e che oggi nessun client prende; chi vuole cedere il
passo guarda `in_use`, che i lock li comprende. La prima metà è una modifica ai client, non ad
Aethera.

## Il reranker vuole una prova, non un `/health`

Qwen non pubblica un GGUF ufficiale del reranker: restano quelli della comunità, e GalaxyCenter
avverte che molti sono rotti — rispondono 200 e poi danno punteggi vicini a zero anche al
documento pertinente. Per questo all'avvio di un servizio `rerank` Aethera fa un test di sanità
vero (`services::rerank_sanity`): una domanda, un documento pertinente e due che non c'entrano. Se
il pertinente non stacca gli altri, il servizio risulta acceso ma **non sano**, e la scheda
GalaxyCenter lo scrive dentro il TOML invece di lasciare che se ne accorga l'indice sbagliato.

**Provato il 20-09, e l'avvertimento era fondato.** Il GGUF della comunità
(`Mungert/Qwen3-Reranker-0.6B-GGUF`, SHA-256 verificato) dà **8,9e-16 al documento pertinente** e
5,5e-12 a uno che non c'entra: non solo è vicino a zero, è pure nell'ordine sbagliato. Prima di
dare la colpa ai pesi è stato escluso un difetto nostro, riprovando con `--pooling rank` esplicito:
punteggi identici, quindi non era un flag mancante.

Convertendo invece i pesi ufficiali `Qwen/Qwen3-Reranker-0.6B` con `convert_hf_to_gguf.py` di
llama.cpp (la copia del fork in `<radice>/src/llama.cpp`, `--outtype q8_0`) il risultato è
**0,9967 al pertinente** e 5,8e-06 e 1,2e-05 agli altri: lo stesso numero che GalaxyCenter riporta
per la sua conversione.

Conseguenza per il prodotto: **il profilo del reranker non è seminato** con l'installazione, a
differenza di quello dell'embedding. Un profilo di serie deve poter portare i suoi pesi, e questi
non si scaricano da nessuna parte — si fanno. Il profilo sta nella radice dati di questa macchina
con dentro la sua storia, e la conversione è questa:

```
python -m venv venv-conv
venv-conv/Scripts/pip install -r <radice>/src/llama.cpp/requirements/requirements-convert_hf_to_gguf.txt
# i pesi ufficiali da huggingface.co/Qwen/Qwen3-Reranker-0.6B
venv-conv/Scripts/python <radice>/src/llama.cpp/convert_hf_to_gguf.py <cartella pesi>   --outfile Qwen3-Reranker-0.6B-conv-Q8_0.gguf --outtype q8_0
```

Vedi [[architettura]], [[considerazioni-aethera-dalle-misure]].
