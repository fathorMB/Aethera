---
updated: 2026-09-20
by: lead
---
**M-18 T-14 chiuso il 20-09 pomeriggio.** Motore spento, VGM 64, nessun giro in corso.

**La divergenza non c'è più, e con lei il motivo di spegnere i checkpoint.** Il sospetto scritto nel task era giusto (`serde_json` riordinava le chiavi degli argomenti; il template di Qwen3.6 li rende nell'ordine ricevuto): Nonio l'ha chiuso con `78b4099` il 19-09 all'1:29, **cinque ore dopo** la nostra misura dell'81,1%. La nota di ieri che lo dava per «smentito» leggeva la cura come prova che la malattia non c'era.

**A-B di oggi** (16 compiti per braccio, Q8, stesso binario `20bc0cfb` da `main`):
- checkpoint **spenti** 14/16, 48,7 compiti/ora, costo fisso **0,43 s**
- checkpoint **accesi** 16/16, 42,8 compiti/ora, costo fisso **0,42 s**
- su 245 richieste, quelle a riuso zero sono **una per compito** in tutti e due i bracci: è la cartella nuova, non una divergenza.

Costo fisso pari → **il −82% non esiste più da incassare**; i checkpoint restano accesi. La differenza fra i bracci (2 compiti, 6 compiti/ora) sta dentro il rumore noto, e i turni oscillano da 104 a 143.

**Due cose lasciate aperte per l'operatore:**
- il repo di Nonio è rimasto su `main` (era su `famiglia-argomenti-strumenti`, pubblicato e non unito);
- il kit ha ancora **due milestone attivi**, M-18 e M-19.

**Aggiunto al runner:** ogni riga registra quale `nonio.exe` ha girato (sha256, mtime, HEAD) e la telemetria richiesta per richiesta con il conto di quelle a riuso zero — senza, oggi non si sarebbe potuto dire con che binario girasse la notte del 20-09.
