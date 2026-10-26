import {createContext} from '@lit/context';
import type {Store} from '@mosaicajs/client';

/**
 * The Lit context through which elements find their store. `<mosaica-store>` and
 * `<mosaica-explorer>` provide it. To supply a store from your own element, provide it under this
 * context with a `ContextProvider` from `@lit/context`. The key is `Symbol.for('mosaica.store')`,
 * so two copies of the package on one page share it.
 */
export const storeContext = createContext<Store | null>(Symbol.for('mosaica.store'));
