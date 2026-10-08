import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
export default defineConfig({
  plugins: [react()],
  server: {
    port: 4311,
    strictPort: true,
    proxy: {
      "/v1": {
        target: process.env.APPS_API_PROXY || "http://127.0.0.1:4310",
        changeOrigin: true,
      },
      "/health": {
        target: process.env.APPS_API_PROXY || "http://127.0.0.1:4310",
      },
    },
  },
  build: {
    sourcemap: false,
    rollupOptions: {
      output: {
        manualChunks: {
          motion: ["motion"],
          vendor: ["react", "react-dom", "react-router-dom"],
        },
      },
    },
  },
});
