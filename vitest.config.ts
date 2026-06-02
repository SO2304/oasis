import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    include: ['kernel/**/__tests__/**/*.test.ts'],
    globals: false,
    testTimeout: 60_000,
  },
});
