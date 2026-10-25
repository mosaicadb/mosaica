import {defaultClientConditions, defineConfig} from 'vite';
import {mosaicaDecorators} from '../../components/vite-plugin-decorators.js';
import {mosaicaDev} from './dev/mosaica.js';

export default defineConfig({
  server: {port: 5310, strictPort: true},
  plugins: [
    // Development only: starts Mosaica over the GeoNames sample, passes the page's requests under
    // /mosaica on to it, and mints viewer tokens at /dev/token.
    mosaicaDev(),
    // Inside this repository only: the page runs on the packages' sources, with no build.
    mosaicaDecorators()
  ],
  resolve: {conditions: ['mosaica-source', ...defaultClientConditions]}
});
