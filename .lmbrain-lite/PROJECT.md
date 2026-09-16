---
title: Project overview
updated: 2026-09-15
---

# Aethera

## What it is

Aethera è un launcher desktop per l'inferenza locale con `llama-server` (llama.cpp), pensato per l'agentic coding con modelli Qwen su hardware a memoria unificata (Minisforum AI X1 Pro, Radeon 890M, 96 GB di cui 48 riservati alla iGPU). Gestisce il ciclo di vita del motore, il catalogo di pesi e build, i profili di avvio, la memoria misurata e la telemetria delle prestazioni. Nasce dai requisiti raccolti in `<minis-config>\docs\13-launcher.md` (2026-09-15).

## Who uses it and why

Un solo operatore (Moreno) su due macchine: Turing (sviluppo, RTX 2080 SUPER 8 GB) e Minisforum (bersaglio). I clienti del motore sono agenti OpenAI-compatibili (Nonio, Diorama, banchi di minis-config). Aethera sostituisce LM Studio perché quello nascondeva la riga di comando, accendeva leve non dichiarate e duplicava i pesi in RAM.

## Stack and how to run it

Tauri 2 + SolidJS (come la finestra di Nonio). Backend Rust: processo figlio in job object, tray, heap Vulkan e contatori GPU, download con SHA-256, endpoint proprio accanto a `llama-server`. Profili in TOML con schema proprio. Dati in una radice scelta dall'utente: `machine.toml`, `profiles/`, `builds/`, `runs/<id>/`, telemetria.

Decisioni di progetto: vedi [[decisioni-di-progetto]]. Moduli, comandi e verifiche: vedi [[architettura]].

## Constraints

- Un processo `llama-server` alla volta, avviato e fermato da Aethera; nessun riavvio implicito, mai mentre un client lavora (la cache del prefisso è la proprietà più importante per un agente).
- Ogni avvio scrive un manifest con la riga di comando esatta, build e commit, pesi e hash, contesto dichiarato e servito, memoria misurata.
- Nessuna leva accesa di nascosto; porta sempre esplicita; alias servito = nome del profilo.
- Assente non è zero: `null` / «sconosciuto», mai una stima spacciata per misura.
- Windows prima; letture di sistema dietro un'interfaccia, nessuna schermata assume Windows.

## Out of scope

- Lanciare banchi di prova o giudicare i task (restano in minis-config e Nonio).
- Decidere il campionamento delle sessioni dei client (lo manda il client).
- Impostare la Variable Graphics Memory (si legge, non si scrive).
- Più motori insieme nella v1 (lo schema non lo impedisce).
