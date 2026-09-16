# La NPU si blocca sotto carico continuo — 16-09-2026

Macchina: Minisforum con Ryzen AI (NPU XDNA2, PCI `VEN_1022&DEV_17F0`, iGPU Radeon 890M), Windows 11 Pro 26200.
Software sulla NPU: FastFlowLM 1.0.5 portatile, `flm serve qwen3.5:4b -e 1 --pmode performance`, modelli
`embed-gemma:300m` e `qwen3.5:4b`.

## Cosa succede

Embedding in continuo, uno alla volta (1.500 caratteri, ~0,62 s ciascuno). Circa una richiesta su 90, senza
legame col testo, la NPU smette di avanzare:

| tempo dall'inizio della richiesta | cosa succede | dove si vede |
|---|---|---|
| 0 s | FLM prende la NPU | log di FLM, «NPU Locked!» |
| ~2,2–2,7 s | lo scheduler di Windows vede che il comando non avanza e resetta il motore | `dxgmms2!VidSchiCheckHwProgress` → `VidSchiResetEngines`; LiveKernelEvent 141 |
| ~7 s | il driver resetta la funzione PCI della NPU | System, evento `pci` 3 su `\Device\NTPNP_PCI0035` |
| ~10 s | la richiesta finisce, senza errori | FLM non registra niente di anomalo |

Il timeout è quello di default di Windows (2 s, `TdrDelay` non impostato): un embedding normale ci sta tre
volte, quindi non è un lavoro lungo ma un blocco del dispositivo.

## Dump

Dodici minidump `WATCHDOG` (copie in `C:\AetheraData\m08\dumps`, analisi con WinDbg 1.2606 in `analisi\`),
tutti con lo stesso responsabile:

- `FAILURE_BUCKET_ID: LKD_0x141_IMAGE_ipustack.sys`
- driver vecchio, `ipustack.sys` 32.0.203.314 (timestamp 11-10-2025): `ipustack+9a50`, nove dump fra 12:49 e 13:09
- driver nuovo, `ipustack.sys` 32.00.20102.3930 (Adrenalin, 16-09): `ipustack+ca40`, tre dump alle 13:33, 13:35 e 13:46
- stack del thread che rileva il blocco, identico in tutti: `dxgmms2!VidSchiWorkerThread` → `VidSchiScheduleCommandToRun`
  → `VidSchiWaitForSchedulerEvents` → `VidSchiCheckHwProgress` → `VidSchiResetEngines` → `VidSchiResetHwEngine`
  → `dxgkrnl!TdrCollectDbgInfoStage1`
- nessuna stringa del firmware nei dati secondari; `ipustack.sys` non ha simboli pubblici, quindi lo stack interno
  del driver non si legge da qui
- resta non letto il dump completo delle 13:09 (`C:\Windows\LiveKernelReports\WATCHDOG-20260916-1309.dmp`,
  14,8 GB): senza simboli del driver aggiungerebbe poco

## Driver vecchio contro nuovo

- **vecchio:** nessun reset PCI, recupero in ~40 s; cinque recuperi fra 12:54 e 12:58, poi schermata blu
  `0x139 KERNEL_SECURITY_CHECK_FAILURE` (parametro 3, lista del kernel corrotta) e riavvio alle 13:00. Il dump
  di quel bugcheck non c'è.
- **nuovo:** reset PCI e recupero in ~10 s, nessuna schermata blu nelle prove brevi; ma il blocco arriva con la
  stessa frequenza.

Il primo 141 in assoluto è delle 12:49:51, alla prima richiesta di embedding mai mandata alla NPU; prima di
FastFlowLM la macchina non ne aveva.

## Cosa non è stato provato

- `--pmode balanced` o `powersaver` al posto di `performance`
- solo generazione (Qwen3.5-4B) senza embedding, per più di pochi secondi
- un altro runtime sulla NPU (per esempio ONNX Runtime con il provider VitisAI dell'AI bundle), per separare
  il driver dai kernel di FastFlowLM

## Per Aethera

Oggi questa NPU non è un secondo motore affidabile per un carico continuo. T-11 di M-08 resta bloccato.
