import {describe, expect, it, vi} from 'vitest';
import {createStore} from '../src/store.js';
import {fakeClock, fixture, headersOf} from './support.js';

/**
 * A store given neither a token nor `authorise` sends no `authorization` header: an open server
 * answers it, and any other server's refusal is the store's, with its reason.
 */
describe('a store with no token', () => {
  const META = fixture('meta.json');

  function server(open: boolean) {
    const sent: (string | undefined)[] = [];
    const fetch = (async (_input: string, init?: RequestInit) => {
      const authorization = headersOf(init)['authorization'];
      sent.push(authorization);
      if (!open && authorization === undefined) {
        return new Response(JSON.stringify({error: 'bad-credential', detail: 'send a session token as a bearer'}), {
          status: 401,
          headers: {'content-type': 'application/json'}
        });
      }
      return new Response(META, {headers: {'content-type': 'application/json'}});
    }) as typeof globalThis.fetch;
    const store = createStore({viewerUrl: 'http://open', clientOptions: {sessionUrl: '', fetch}, clock: fakeClock(), prefetch: false});
    return {store, sent};
  }

  it('reads /v1/meta from an open server, sending no authorization header', async () => {
    const {store, sent} = server(true);
    await vi.waitFor(() => expect(store.get('meta')).not.toBeNull());
    expect(sent.length).toBeGreaterThan(0);
    expect(sent.every((header) => header === undefined)).toBe(true);
    expect(store.get('status').refusal).toBeNull();
    store.dispose();
  });

  it('shows the refusal of a server that is not open, with its reason', async () => {
    const {store} = server(false);
    await vi.waitFor(() => expect(store.get('status').status).toBe('refused'));
    expect(store.get('status').refusal).toMatchObject({code: 'bad-credential', detail: 'send a session token as a bearer'});
    expect(store.get('meta')).toBeNull();
    store.dispose();
  });
});
