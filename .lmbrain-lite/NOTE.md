---
updated: 2026-09-20
by: lead
---
**20-09 notte: M-20 chiuso 11/11 e pubblicato. Da approvare: M-23.**

**M-20 fatto e su `origin/main`** (`dae066d`, `399d24a`). Aethera accende motori di servizio accanto al principale; GalaxyCenter ha i tre endpoint che pretende, **14 controlli su 14**.

**Le misure hanno smentito lo studio due volte su tre, ed è il motivo per cui si misura:**
- VRAM libera **17,24 GiB**, non ~12. E `--list-devices` non serve: dà lo stesso «free» a motore spento e carico.
- Un secondo motore **carico e fermo è gratis** (+1,8% di prefill); due motori che **lavorano** insieme costano **−58%**, cioè più della NPU. La precedenza al coding è una necessità, non una cortesia.
- **Un modello più piccolo non è più veloce.** Byte letti per token: 35B-A3B Q4 **2,265** · gemma-4-E4B q4_0 **2,277** · 35B-A3B Q8 **3,276** · Qwen3.5-4B Q8 **3,936**. Il 4B-effettivo di Gemma legge quanto il MoE da 35B; un denso da 4B a Q8 legge il 20% in più. Rimpicciolire non compra velocità in nessuna famiglia.

**Tre difetti trovati e corretti**, due miei (leve di generazione mancanti ai servizi `chat`; `--list-devices` come metodo in T-01) e uno vecchio (`m08_bytes` contava `per_layer_token_embd` di Gemma «E» come denso: 4,589 GB/token invece di 2,277).

**M-23 proposto — la cosa che promette di più adesso.** llama.cpp è a **b11064**, 255 commit dopo la nostra b10809, e in mezzo c'è un gruppo **Vulkan+MoE**: `mul_mat` a m=1 per Qwen (il decode), fusione `topk_moe`, skip del lavoro MoE inutile, e il limite esperti da 256 a 512. **Cinque di questi sono già nelle build b10991 che hai sul disco** e il profilo non le usa: T-01 è un A-B che non richiede di compilare niente.

E tocca una decisione passata: il commit b11029 nomina **Qwen3.8-Flash-Next**, che ha 10/**512** esperti — scartato da M-10/M-11 mentre girava su un percorso pessimizzato dal backend.

**Da te:** approvare M-23. E dirmi dove mettere `minisforum-qwen3.5-4b-q8.toml`, rimasto non committato in Nonio.
