---
updated: 2026-09-19
by: lead
---
**Sessione chiusa dall'operatore il 19-09 all'1:15.** Sta usando il motore principale. Macchina a **VGM 64**.

## Stato
- **Aethera 0.2.0 pubblicata** (release su GitHub) e **installata** in `AppData\Local\Programs\Aethera`: finestra v2, profilo principale, finestra che non si blocca più.
- **Profilo principale: G1 Q8_0** — `qwen3.6-35b-a3b.q8_0.vulkan` (VGM 64, ctx 262144) e `…vgm48` (ctx 131072). In Nonio: `profiles/minisforum-qwen3.6-35b-a3b-q8.toml` e `-q8-vgm48.toml` (pubblicati).
- **Nonio `main`** pubblicato con le tre correzioni (+30 %). Il ramo **`aethera/m18-riuso`** (turno troncato, chiamate malformate, `send_reasoning`; 619 test verdi) è **locale e non unito**.

## Report
- `design/giornata-2026-09-18` — Q8 vince, contesto lungo senza errori fino a 200k, n-grammi e checkpoint spenti bocciati dal lavoro vero.
- `design/notte-2026-09-18` — il costo fisso è copia dei checkpoint.

## Prossimo lavoro (M-18)
1. **T-14**: la divergenza residua di Nonio (riuso 92,6 → 81,1 % senza checkpoint). Sospetto: argomenti delle chiamate ri-serializzati. Se si chiude, i checkpoint spenti valgono il −82 %.
2. **T-05**: batteria intera col thinking acceso (una notte): profilo «intelligente»?
3. **T-10**: compito a contesto lungo che tocchi molti file (quello attuale si risolve in 6 turni).
4. **T-15**: chiave di registro lasciata dal disinstallatore.
5. **T-01**: TDP nel BIOS al prossimo riavvio.
6. Decidere se unire il ramo del riuso di Nonio.

## Da sapere
- Le misure lunghe si fanno di notte, dopo un riavvio (il riavvio vale l'8 % di decode; 5 ore di prefill profondi ne tolgono il 10,7 %).
- Pesi aggiunti fuori dall'app: aprire il Catalogo una volta, così li registra.
