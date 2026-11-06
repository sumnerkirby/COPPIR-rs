import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Served by the Rust binary under /v2/. In development, `npm run dev` proxies
// the API and websockets to a server started with `cargo run -- --headless`.
export default defineConfig({
  base: '/v2/',
  plugins: [svelte()],
  build: { outDir: 'dist', emptyOutDir: true, chunkSizeWarningLimit: 1500 },
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
