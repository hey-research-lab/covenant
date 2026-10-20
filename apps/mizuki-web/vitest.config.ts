import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  resolve: {
    alias: {
      'next/font/google': fileURLToPath(
        new URL('./test/next-font-google-stub.ts', import.meta.url),
      ),
      'next/font/local': fileURLToPath(new URL('./test/next-font-local-stub.ts', import.meta.url)),
      '@': fileURLToPath(new URL('.', import.meta.url)),
    },
  },
});
