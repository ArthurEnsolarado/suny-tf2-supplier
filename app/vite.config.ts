import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Porta fixa: o devserver do núcleo (crates/suny-core/examples/devserver.rs) só aceita
// pedidos vindos de http://localhost:1420.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, host: "127.0.0.1" },
  build: { target: "es2022" },
});
