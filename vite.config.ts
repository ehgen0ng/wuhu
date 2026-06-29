import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    proxy: {
      "/steam-api": {
        target: "https://store.steampowered.com",
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/steam-api/, ""),
      },
      "/hubcap-api": {
        target: "https://hubcapmanifest.com",
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/hubcap-api/, ""),
      },
    },
  },
});
