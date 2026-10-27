import {defaultClientConditions} from 'vite';
import {defineConfig} from 'vitest/config';
import {mosaicaDecorators} from './vite-plugin-decorators.js';

export default defineConfig({
  plugins: [mosaicaDecorators()],
  resolve: {conditions: ['mosaica-source', ...defaultClientConditions]},
  test: {environment: 'happy-dom', include: ['test/**/*.test.ts']}
});
