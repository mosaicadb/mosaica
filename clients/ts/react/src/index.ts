/**
 * `@mosaicajs/react` connects the store from `@mosaicajs/client` to React. {@link useMosaicaStore}
 * creates a store that lives as long as your component. {@link useProjection} reads one of the
 * store's projections, the read-only snapshots it publishes of the map's state, and re-renders
 * your component when that projection changes.
 *
 * The elements of `@mosaicajs/components`, such as the map, are React components in the
 * `@mosaicajs/react/components` entry, and you can pass them a store as the `store` prop. This
 * entry imports neither Lit nor deck.gl, so a page that uses only the hooks does not load them.
 *
 * @example
 * ```tsx
 * import {useMosaicaStore, useProjection} from '@mosaicajs/react';
 * import {MosaicaMap} from '@mosaicajs/react/components';
 *
 * function Map({token}: {token: string}) {
 *   const store = useMosaicaStore({viewerUrl: 'http://localhost:8080', token});
 *   const status = useProjection(store, 'status');
 *   return (
 *     <>
 *       <MosaicaMap store={store} />
 *       <p>{status?.status}</p>
 *     </>
 *   );
 * }
 * ```
 *
 * @module @mosaicajs/react
 */
import {useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore} from 'react';
import {createStore, type ProjectionName, type Projections, type Store, type StoreOptions, type TokenSupplier} from '@mosaicajs/client';

/** The options of {@link useMosaicaStore}, which are those of {@link createStore}. */
export type UseMosaicaStoreOptions = Omit<StoreOptions, 'authorise'> & {
  /**
   * A function that returns a viewer token, which the store calls at the start and before each
   * token expires. An inline function is fine: the hook calls the latest one and does not create a
   * new store when it changes. A store serves one viewer. To show the map to another, call the
   * store's `clear()` or give the component a new `key`.
   */
  authorise?: TokenSupplier;
};

/**
 * Creates a store with {@link createStore} for the lifetime of the component, and disposes of it on
 * unmount. The store is created in an effect, so the first render returns `null`.
 *
 * The store is created again when `viewerUrl`, `client`, `token` or `view` changes, or when
 * `authorise` is added or removed. Other options take effect the next time it is created. If you
 * pass a `client`, keep it the same object across renders, for example with `useMemo`, or every
 * render creates a new store. Options that `createStore` refuses, such as neither `viewerUrl` nor
 * `client`, throw from the effect.
 */
export function useMosaicaStore(options: UseMosaicaStoreOptions): Store | null {
  const [store, setStore] = useState<Store | null>(null);
  const latest = useRef(options);
  useLayoutEffect(() => {
    latest.current = options;
  });
  const {viewerUrl, client, token, view} = options;
  const hasAuthorise = options.authorise !== undefined;
  useEffect(() => {
    const opts = latest.current;
    const built = createStore({
      ...opts,
      ...(hasAuthorise ? {authorise: () => latest.current.authorise!()} : {})
    });
    setStore(built);
    return () => {
      built.dispose();
      setStore((current) => (current === built ? null : current));
    };
  }, [viewerUrl, client, token, view, hasAuthorise]);
  return store;
}

const noop = () => {};
const unsubscribed = () => noop;

/**
 * Returns one of the store's projections, and re-renders the component whenever that projection
 * changes. Changes to other projections do not re-render it.
 *
 * @param store - The store, such as the one {@link useMosaicaStore} returns.
 * @param name - The projection to read, such as `marks` or `legend`.
 * @returns The projection, or `null` while `store` is `null`, as it is on the first render.
 */
export function useProjection<K extends ProjectionName>(store: Store, name: K): Projections[K];
export function useProjection<K extends ProjectionName>(store: Store | null, name: K): Projections[K] | null;
export function useProjection<K extends ProjectionName>(store: Store | null, name: K): Projections[K] | null {
  const subscribe = store ? (onChange: () => void) => store.subscribe(name, onChange) : unsubscribed;
  const getSnapshot = store ? () => store.get(name) : () => null;
  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}

export type {ProjectionName, Projections, Store, StoreOptions, TokenSupplier} from '@mosaicajs/client';
