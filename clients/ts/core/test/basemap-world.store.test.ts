import {describe, expect, it, vi} from 'vitest';
import {createStore} from '../src/store.js';
import {fakeClock, fixture, headersOf, settle} from './support.js';

/**
 * The world outline through the store: asked for under the store's token like every viewer
 * request, one answer shared by the stores that reach one server, asked again under a renewed
 * token, and a refusal not kept.
 */
describe('Store.basemapWorld', () => {
  const META = fixture('meta.json');
  const WORLD = new Uint8Array([77, 66, 87, 49, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).buffer;

  /** A server whose sessions are the tokens it issued since it last started. */
  function server(url: string) {
    const s = {asked: [] as string[], sessions: new Set<string>(), issued: 0, refuse: false};
    const fetch = (async (input: string, init?: RequestInit) => {
      const route = new URL(input).pathname;
      const token = headersOf(init)['authorization']?.slice(7) ?? '';
      s.asked.push(`${route} ${token}`);
      if (!s.sessions.has(token)) return new Response(JSON.stringify({error: 'bad-credential', detail: 'no such session'}), {status: 401, headers: {'content-type': 'application/json'}});
      if (route === '/v1/meta') return new Response(META, {headers: {'content-type': 'application/json'}});
      if (route === '/v1/basemaps/world') {
        if (s.refuse) return new Response(JSON.stringify({error: 'expired-token', detail: 'the session has ended'}), {status: 403, headers: {'content-type': 'application/json'}});
        return new Response(WORLD.slice(0), {headers: {'content-type': 'application/octet-stream'}});
      }
      return new Response('{}', {status: 404});
    }) as typeof globalThis.fetch;
    const store = (token: string) => {
      s.sessions.add(token);
      return createStore({viewerUrl: url, token, clientOptions: {fetch}, clock: fakeClock(), prefetch: false});
    };
    const authorise = vi.fn(async () => {
      const token = `t${++s.issued}`;
      s.sessions.add(token);
      return {token, expiresAt: Date.now() / 1000 + 3600};
    });
    const authorised = () => createStore({viewerUrl: url, authorise, clientOptions: {fetch}, clock: fakeClock(), prefetch: false});
    return {s, store, authorised, authorise, world: () => s.asked.filter((a) => a.startsWith('/v1/basemaps/world'))};
  }

  it('is asked for once per server, with a token, whichever viewer asks', async () => {
    const srv = server('http://world-a');
    const [a, b] = [srv.store('t1'), srv.store('t2')];
    const [ab, bb] = await Promise.all([a.basemapWorld(), b.basemapWorld()]);
    expect(ab).toBe(bb);
    expect(new Uint8Array(ab)).toEqual(new Uint8Array(WORLD));
    expect(srv.world()).toHaveLength(1);
    expect(srv.world()[0]).toMatch(/ t[12]$/);
    a.dispose();
    b.dispose();
  });

  it('rejects a refusal and asks again on the next call', async () => {
    const srv = server('http://world-b');
    const store = srv.store('t1');
    srv.s.refuse = true;
    await expect(store.basemapWorld()).rejects.toMatchObject({code: 'expired-token'});
    srv.s.refuse = false;
    await expect(store.basemapWorld()).resolves.toBeInstanceOf(ArrayBuffer);
    expect(srv.world()).toHaveLength(2);
    store.dispose();
  });

  it('is asked again under the renewed token after a 401 on one that had worked', async () => {
    const srv = server('http://world-c');
    const store = srv.authorised();
    // The store's own read of meta makes the first token one that has worked.
    await settle();
    await vi.waitFor(() => expect(store.get('meta')).not.toBeNull());
    // A restart that keeps no sessions.
    srv.s.sessions.clear();
    await expect(store.basemapWorld()).resolves.toBeInstanceOf(ArrayBuffer);
    expect(srv.world()).toEqual(['/v1/basemaps/world t1', '/v1/basemaps/world t2']);
    store.dispose();
  });

  it('shares a success only: a store waiting on one that fails asks for itself', async () => {
    const srv = server('http://world-d');
    // The first store's token arrives only after it has been disposed, so its request fails.
    let release!: () => void;
    const gate = new Promise<void>((resolve) => (release = resolve));
    const first = createStore({
      viewerUrl: 'http://world-d',
      authorise: async () => {
        await gate;
        srv.s.sessions.add('late');
        return {token: 'late', expiresAt: Date.now() / 1000 + 3600};
      },
      clientOptions: {fetch: (async () => new Response('{}', {status: 500})) as typeof globalThis.fetch},
      clock: fakeClock(),
      prefetch: false
    });
    const second = srv.store('t2');
    const firstAsk = first.basemapWorld();
    const secondAsk = second.basemapWorld();
    first.dispose();
    release();
    await expect(firstAsk).rejects.toThrow();
    await expect(secondAsk).resolves.toBeInstanceOf(ArrayBuffer);
    expect(srv.world()).toEqual(['/v1/basemaps/world t2']);
    second.dispose();
  });
});
