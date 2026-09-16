---
updated: 2026-09-17
by: lead
---
**Priorità della notte 16→17-09: M-14, fork leggero di llama.cpp** (approvato dall'operatore in chat).

- Quando la macchina è libera dalle misure a VGM 64 di M-10, si passa M-14 a un sub-agent Opus.
- Toolchain: Build Tools 2022 con MSVC 14.44, CMake e Ninja ci sono. **Manca il Vulkan SDK**: senza, niente build (T-04…T-06).
- Le misure di T-04/T-06 si confrontano con M-08, quindi vanno fatte **a VGM 48**.

**In corso**
- M-10 a VGM 64: T-08 all'ultimo giro, poi T-09.
- M-12: sub-agent sulla finestra v2.

**Fatto stasera**
- M-13: release di prova v0.1.0-rc.1 in bozza; installatore provato dall'operatore. Bozza e tag restano fino alla v0.1.0.
- Repo pubblico, percorsi tolti (PR #2); `.lmbrain-lite/m10` segue a misure finite.

**Tu**
- Vulkan SDK.
- VGM a 48 a fine misure.
- T-07 di M-13 quando vuoi la v0.1.0.
