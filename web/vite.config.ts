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

const src = (dir: string) => path.resolve(__dirname, './src', dir);

export default defineConfig({
  base: '/',
  plugins: [react()],
  // Абсолютные импорты от src (tsconfig.app.json → paths).
  resolve: {
    alias: {
      common: src('common'),
      entities: src('entities'),
      layouts: src('layouts'),
      providers: src('providers'),
      styles: src('styles'),
    },
  },
  css: {
    preprocessorOptions: {
      scss: {
        // Позволяет писать @use 'breakpoints' из любого .module.scss.
        loadPaths: [src('styles')],
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
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/test-setup.ts'],
    include: ['src/**/*.test.{ts,tsx}'],
    css: { modules: { classNameStrategy: 'non-scoped' } },
    coverage: {
      provider: 'v8',
      include: ['src/**/*.{ts,tsx}'],
      // Вне покрытия: тесты и тестовая обвязка, моки, корень (main.tsx, app.tsx),
      // типы без кода (models.ts, *.types.ts), реэкспорты (index.ts) и тексты (*.consts.ts).
      exclude: [
        'src/**/*.test.{ts,tsx}',
        'src/test-setup.ts',
        'src/common/test/**',
        'src/**/mocks/**',
        'src/main.tsx',
        'src/app.tsx',
        'src/vite-env.d.ts',
        'src/**/index.ts',
        'src/**/*.consts.ts',
        'src/**/*.types.ts',
        'src/**/models.ts',
        'src/common/types/**',
      ],
      // Общий порог обоих проектов (Session Search и VPN Closer).
      thresholds: { lines: 90, statements: 90, functions: 90, branches: 85 },
    },
  },
});
