# Fork leggero di llama.cpp (M-14)

Build di llama.cpp compilate su questa macchina: un tag di ggml-org più una serie corta di patch,
ognuna nel suo ramo. Non è un motore nostro, e non deve diventarlo.

Questo file è la fonte delle regole: `build.ps1` lo copia a ogni build nella radice del repo del fork
come `LEGGIMI-MORO.md` (escluso da git con `.git/info/exclude`).

## Dove sta

| cosa | dove |
|---|---|
| repo del fork | `<radice>\src\llama.cpp`, **fuori** da questo repository |
| script di build | `.lmbrain-lite/fork/build.ps1` (in questo repository) |
| build | `<radice>\builds\llama-b<numero>+moro<n>-win-vulkan-x64` |
| provenienza | `provenienza.toml` accanto a `llama-server.exe` |
| cartelle di build | `<radice>\src\llama.cpp\build-moro\b<numero>+moro<n>` (ignorate da git) |

## Le tre regole

1. **Master resta la base.** Ogni patch sta in un ramo suo (`patch/<nome>`), piccolo, ribasato sul tag
   a ogni build adottata, e punta a sparire: fusa upstream o abbandonata. **Un ramo che non ribasa più
   si butta**: è il segnale che si sta costruendo il motore custom che abbiamo deciso di non fare.
   Un conflitto si risolve solo se è meccanico e sicuro; non si riscrivono kernel per far entrare una patch.
2. **Ogni build dichiara che cosa è.** L'id è `b<numero>+moro<n>-<backend>`: `moro0` è il tag liscio
   compilato qui, `moro<n>` con n ≥ 1 è una serie con patch. `provenienza.toml` elenca tag, commit,
   rami e loro commit, data e durata della build. Aethera lo copia nel manifest di ogni avvio e tratta
   la serie come una condizione (come il driver): Benchmark non confronta una build patchata con una
   liscia senza dirlo. Un profilo usa una build patchata solo se la chiede per nome
   (`build = "b10991+moro1"`); un profilo su `b10991` non ci ricade mai.
3. **Una patch entra solo con una misura.** Procedura di M-08: stessa base, una variabile per volta,
   giri ripetuti (3–5), prompt congelati (7k e 21k), prefill e decode, memoria dopo il caricamento,
   e fedeltà numerica misurata come sotto. Senza guadagno misurato si butta. La fedeltà si giudica
   **modello per modello**: una patch può entrare in `moro-ai` e servire solo ai profili dei modelli
   per cui passa.

## Regola di fedeltà (M-16)

Il testo identico a temperatura 0 **non è più un requisito**. M-16 ha misurato che cambiare solo
l'ubatch, a parità di build, cambia già le distribuzioni dei token e il testo generato (G1 e G3:
quasi sempre 0 giri identici su 3, spesso allo stesso carattere in cui diverge la patch): un
criterio che scarta ogni cambiamento numerico scarterebbe anche un cambio di profilo innocuo. Il testo si registra comunque (banco con `fixed_nonce` e `save_text`), come informazione.

**Come si misura.** `llama-perplexity` della base con `--kl-divergence-base`, poi con
`--kl-divergence` (script `.lmbrain-lite/m16/sequenza.py`, fase `kld-*`): prompt congelato da 21k,
contesto 8192 (2 blocchi, 8.190 token valutati), `-ngl 999 -fa on`, cache f16, ubatch e batch del
profilo. Per ogni modello si misurano:

- **pavimento**: la base contro se stessa (deve dare KLD 0 e 100 % di stesso primo token; se no la
  base non è deterministica e la misura non vale);
- **metro dell'ubatch**: la base con un altro ubatch (G1 512 invece di 4096, G3 2048 invece di 512).
  Misura quanto cambiano i conti con gli stessi kernel tagliati diversamente;
- **metro del backend**, quando il modello sta in RAM: la stessa base calcolata dal backend CPU
  (`-dev none -ngl 0`, fase `cpu-g1`). Misura quanto differiscono due implementazioni corrette.
  Una patch che cambia il percorso numerico (int8 invece di f16) va confrontata con questo, non solo
  con l'ubatch: sul G1 il metro dell'ubatch è minuscolo perché i kernel restano gli stessi;
- **patch**: la build patchata all'ubatch del profilo (e, per controllo, all'altro ubatch);
- **picco**: per la patch, `kld_per_token.py` sui token peggiori (dove cadono, se sono isolati o a
  gruppi, se sono quasi-pareggi) e `nll_zona.py` sulla zona (quale calcolo segue meglio il testo
  vero dove i calcoli non sono d'accordo).

**Soglie** (per modello, patch contro base allo stesso ubatch; «metro» è il più grande dei due metri
disponibili):

| misura | soglia | perché |
|---|---|---|
| Δ perplessità | dentro 2 σ, o negativa | una patch che peggiora la perplessità non entra |
| KLD media | ≤ 2 × metro, e comunque ≤ 0,02 | il metro è quanto la base cambia da sola senza che nessuno la consideri sbagliata |
| KLD al 99 % | ≤ 2 × metro al 99 % | le code contano più della media: sono i token dove la risposta si biforca |
| KLD massima | ≤ 1,0, oppure in una zona dove anche il metro del backend si discosta dalla base e dove la NLL del token vero con la patch non è peggiore di quella della base | un token con KLD ≥ 1 è una distribuzione diversa, non un arrotondamento: va spiegato |
| stesso primo token | ≥ metro − 1 punto | |
| batteria di M-15 | riusciti ≥ riferimento − 2 su 15, nessun file protetto toccato, nessuna causa di fallimento nuova; riferimento sulla stessa base e con lo stesso ubatch | un giro solo ha rumore di ±1–2 compiti |

Con i numeri di M-16 (rapporto `reports/int8-coopmat-2026-09.md`, prompt da 21k, 8.190 token):

| modello | metro | KLD media | KLD 99 % | KLD max | stesso primo token |
|---|---|---:|---:|---:|---:|
| G1 | ubatch 512 contro 4096 | 0,00066 | 0,0064 | 0,020 | 98,9 % |
| G1 | CPU contro Vulkan (moro0) | 0,018 | 0,101 | 12,8 | 97,5 % |
| G3 | ubatch 2048 contro 512 | 0,0065 | 0,082 | 0,88 | 97,2 % |
| G3 | CPU | non misurabile: 48,5 GB di pesi, 34 GiB di RAM libera | | | |

Le soglie si ricalcolano quando cambia il tag di base: i metri vanno rimisurati sul tag nuovo.

## Regola di adozione

- Una patch entra nel ramo `moro-ai` **solo** con una misura che la giustifica, scritta in un
  rapporto (`.lmbrain-lite/reports/`), con il numero di serie che la contiene.
- Le build patchate **non** sostituiscono quelle di ggml-org nei profili standard (G1, G3): si usano
  in profili di prova finché un rapporto non dice altro, e il cambio lo decide l'operatore.
- A ogni nuovo tag adottato si ribasano tutti i rami. Se una patch è stata fusa upstream il ramo si
  cancella; se non ribasa più si cancella; se la misura sul nuovo tag non regge più si cancella.
- Niente su GitHub finché non c'è una patch nostra da proporre upstream: il remote `upstream` non ha
  un indirizzo di push.

## Rami

| ramo | che cosa è |
|---|---|
| `master` | copia di `upstream/master`, non ci si lavora |
| `moro-ai` | ricostruito da `build.ps1` a ogni build: tag + commit delle patch, nell'ordine dato |
| `patch/<nome>` | una patch, ribasata sul tag dell'ultima build |

`moro-ai` non si modifica a mano: la serie si cambia cambiando l'elenco dei rami passato allo script.

## Patch candidate

| # | ramo | origine | stato al 17-09-2026 | note |
|---|---|---|---|---|
| 1 | `patch/int8-coopmat` | PR ggml-org/llama.cpp#27952 (int8 coopmat1 per il prefill, RDNA3/RDNA4) | PR aperta; misurata in M-14, verdetto nel rapporto `fork-leggero-2026-09.md` | la 890M è riconosciuta come RDNA3 (dot product int8 accelerato), quindi il percorso si accende |
| 2 | `patch/lazy-readahead` | follow-up della PR #27794 (lettura a lotti per `--lazy-mode`) | da scrivere | serve a M-10 |
| 3 | `patch/mtp-flash-next` | PR #28243 (MTP per Qwen3.8-Flash-Next) | bozza, in conflitto con master | serve a M-10 T-11; entra solo se ribasa in modo meccanico |
| 4 | `patch/mmid-gfx1150` | nostra: forme dei tile e subgroup di `mul_mat_id` sulla 890M | dopo una profilazione con il logger di prestazioni Vulkan di ggml | l'unica che scriveremmo noi |

Non candidate: backend NPU (mesi di lavoro, e la NPU al lavoro costa il 40 % al motore principale),
ROCm/HIP (la gfx1150 non vede la memoria condivisa).

## Preparare il repo (una volta)

```bash
git clone --filter=blob:none --origin upstream https://github.com/ggml-org/llama.cpp.git <radice>/src/llama.cpp
cd <radice>/src/llama.cpp
git remote set-url --push upstream NESSUN-PUSH-fork-solo-locale
git branch -f moro-ai b<numero>
echo LEGGIMI-MORO.md >> .git/info/exclude
# una patch da una PR:
git fetch upstream pull/<numero-PR>/head:patch/<nome>
```

## Fare una build

Da PowerShell, con la radice dati in `AETHERA_RADICE` o passata con `-Radice`:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .lmbrain-lite\fork\build.ps1 -Tag b10991 -Serie 0
powershell -NoProfile -ExecutionPolicy Bypass -File .lmbrain-lite\fork\build.ps1 -Tag b10991 -Patch patch/int8-coopmat -Serie 1
```

Lo script trova Build Tools con `vswhere`, carica l'ambiente di `VsDevCmd.bat -arch=x64` e usa CMake e
Ninja inclusi in Build Tools; il Vulkan SDK deve essere in `VULKAN_SDK`. Codici d'uscita: 0 fatto,
1 errore, 2 conflitto di rebase o di cherry-pick (ramo lasciato com'era).

Opzioni di cmake: `GGML_VULKAN=ON`, `GGML_NATIVE=OFF`, `GGML_BACKEND_DL=ON`,
`GGML_CPU_ALL_VARIANTS=ON`, `LLAMA_BUILD_NUMBER=<numero del tag>`, test ed esempi spenti, interfaccia
web spenta (`LLAMA_USE_PREBUILT_UI=OFF`: niente download durante la build; Aethera usa solo le API).

**Differenze note dalla release di ggml-org**, che T-04 ha misurato: la release compila la parte CPU
con clang (varianti zen4 comprese, `libomp`) e la parte Vulkan con MSVC; qui è tutto MSVC, quindi le
varianti CPU sono meno (la più alta è AVX-512 «icelake»), e mancano interfaccia web e HTTPS. Con tutti
i layer sulla GPU la parte CPU non pesa sui numeri: vedi il rapporto.
