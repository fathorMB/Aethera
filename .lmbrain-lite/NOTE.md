---
updated: 2026-09-16
by: lead
---
**M-08 in corso, notte del 16-09.** Approvato a voce; procedo da solo. Stato vivo: `.lmbrain-lite/m08/` (scenari, prompt congelati, strumenti) e `C:\AetheraData\m08\` (righe grezze). Ogni misura è un avvio di Aethera da profilo: il run id in `runs/` è la prova.

## Già misurato

- **T-02 banda.** Il denso Qwen3-8B fa 13,42 tok/s leggendo 4,672 GB per token = **62,7 GB/s**, dentro la finestra 60-67 che lo studio dava come tetto della piattaforma. Il MoE 35B fa 21,83 tok/s leggendo 2,265 GB per token = **49,4 GB/s**, il 79% di quel tetto. Quindi **il limite non è la macchina**: il 21% che manca al MoE è costo dei kernel e del router. Il «~42 GB/s» dello studio era sottostimato e va corretto.
- **T-03 SSD.** 2 MiB sequenziale QD8: 4.378 MB/s (dichiarati 5.700). 4 KiB casuale QD32: 296 MB/s. Un'anomalia da rifare: a 2 MiB *casuale* la banda scende da 1.952 (QD1) a 771 MB/s (QD8). Il disco aveva appena assorbito 53 GB di download: rifaccio il giro a disco fermo prima di scrivere il verdetto.
- **Scaricati e verificati** (codice del Catalogo, hash confrontato): build b10991, Qwen3-8B Q4_K_M, Qwen3-Coder-Next Q4_K_M 48,53 GB.

## Rilievo che ti riguarda

`--spec-draft-adaptive` del piano **non esiste** in b10809: le leve vere sono `--spec-draft-n-min/n-max/p-min`. T-05 le misura con 2 / 4 / 0,75.

## Ti servirà decidere

**T-08 VGM a 64 GB** chiede la GUI Adrenalin e un riavvio: non lo faccio da solo di notte. Misuro tutto il resto a 48.

**T-11 NPU**: FastFlowLM è MIT per la CLI ma i kernel NPU sono binari proprietari, liberi solo per uso non commerciale o per aziende **sotto i 10 M$ di fatturato**. Uso la versione portatile (zip, nessuna installazione); la condizione sul fatturato la puoi confermare solo tu.
