/// <reference types="vitest/config" />
import path from 'node:path';

import react from '@vitejs/plugin-react-swc';
import { defineConfig } from 'vite';

// Куда проксировать /api в dev/preview. 127.0.0.1, не localhost: localhost
// сначала идёт в IPv6, а там на том же порту может сидеть чужой сервер.
const API_TARGET = process.env.SESSION_SEARCH_API ?? 'http://127.0.0.1:4477';

const proxy = {
  '/api': { target: API_TARGET, changeOrigin: true },
};

export default defineConfig({
  base: '/',
  plugins: [react()],
  css: {
    preprocessorOptions: {
      scss: {
        // Позволяет писать @use 'breakpoints' из любого .module.scss.
        loadPaths: [path.resolve(__dirname, './src/styles')],
      },
    },
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    chunkSizeWarningLimit: 1500,
  },
  server: { host: '127.0.0.1', proxy },
  preview: { host: '127.0.0.1', proxy },
  test: {
    include: ['src/**/*.test.ts'],
  },
});
