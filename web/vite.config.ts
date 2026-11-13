import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Served by the Rust binary at /. In development, `npm run dev` proxies the
// API and websockets to a server started with `cargo run -- --headless`.
export default defineConfig({
  base: '/',
  plugins: [svelte()],
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    chunkSizeWarningLimit: 1500,
    rollupOptions: {
      output: {
        // Keep the big libraries in their own files: they change rarely, so
        // browsers keep them cached across app updates.
        manualChunks(id: string) {
          if (id.includes('node_modules/maplibre-gl')) return 'maplibre';
          if (id.includes('node_modules/lucide')) return 'icons';
          if (id.includes('node_modules/terra-draw')) return 'draw';
        },
      },
    },
  },
  server: {
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:8000',
        ws: true,
        changeOrigin: true,
        // The server refuses foreign Origins (src/guard.rs). The dev server
        // is a different origin, so present the request as same-origin.
        configure: (proxy) => {
          const sameOrigin = (req: { setHeader(k: string, v: string): void }) =>
            req.setHeader('origin', 'http://127.0.0.1:8000');
          proxy.on('proxyReq', sameOrigin);
          proxy.on('proxyReqWs', sameOrigin);
        },
      },
    },
  },
});
