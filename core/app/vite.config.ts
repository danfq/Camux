import react from "@vitejs/plugin-react";
import { defineConfig, type Plugin } from "vite";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath } from "node:url";

const tauriControlsLayer: Plugin = {
  name: "tauri-controls-css-layer",
  enforce: "pre",
  transform(source, id) {
    if (id.endsWith("/node_modules/tauri-controls/dist/style.css")) {
      return `@layer tauri-controls {\n${source}\n}`;
    }
  },
};

// https://vite.dev/config/
export default defineConfig({
  plugins: [tauriControlsLayer, react(), tailwindcss()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
  },
});
