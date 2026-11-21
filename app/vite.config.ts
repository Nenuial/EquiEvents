import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

// In the desktop app the UI talks to the Rust core directly. In a browser (development) it goes
// through `office-dev-server` (cargo run -p office --bin office-dev-server -- <library>).
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    proxy: { '/api': 'http://127.0.0.1:7878', '/files': 'http://127.0.0.1:7878' },
  },
});
