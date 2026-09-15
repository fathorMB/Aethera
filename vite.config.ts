import solid from "vite-plugin-solid";
import { defineConfig } from "vitest/config";

// Tauri serve i file da dist/ in build e da questo server in sviluppo.
// Nessuna risorsa dalla rete a runtime: i font sono quelli di sistema.
export default defineConfig({
  plugins: [solid()],
  clearScreen: false,
  server: {
    port: 5283,
    strictPort: true,
    host: "127.0.0.1",
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: "chrome120",
    sourcemap: true,
    emptyOutDir: true,
  },
  // I test del frontend sono logica pura: nessun DOM simulato.
  test: {
    environment: "node",
  },
});
