import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import { resolve } from "node:path";

export default defineConfig({
  plugins: [react()],
  worker: {
    format: "es",
  },
  server: {
    host: "127.0.0.1",
    port: 5173,
  },
  publicDir: "public",
  build: {
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        waveDemo: resolve(__dirname, "wave-demo.html"),
      },
    },
  },
});
