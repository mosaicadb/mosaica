import {describe, expect, it, vi} from 'vitest';
import {createStore} from '../src/store.js';
import {camera, fakeClock, fakeScheduler, fixture, headersOf, settle} from './support.js';

/**
 * A server that goes away under a store and comes back without the sessions it had, as a restart
 * leaves it: the store says it cannot reach the server, keeps asking, and on the way back asks
 * `authorise` for a new token and asks again for what is drawn.
 */
describe('a server that goes away and comes back', () => {
  const META = fixture('meta.json');
  const VIEWPORT = fixture('viewport-plain.bin');

  /** A server whose sessions are the tokens it issued since it last started, and which can be stopped. */
  function server() {
    const s = {up: true, sessions: new Set<string>(), issued: 0, asked: [] as string[]};
    const fetch = (async (url: string, init?: RequestInit) => {
      if (!s.up) throw new TypeError('fetch failed');
      const route = new URL(url).pathname;
      s.asked.push(route);
      const token = headersOf(init)['authorization']?.slice(7) ?? '';
      if (!s.sessions.has(token)) return new Response(JSON.stringify({error: 'bad-credential', detail: 'no such session'}), {status: 401, headers: {'content-type': 'application/json'}});
      if (route === '/v1/meta') return new Response(META, {headers: {'content-type': 'application/json'}});
      if (route === '/v1/viewport') return new Response(VIEWPORT.slice(), {headers: {etag: '"ck"', 'x-mosaica-identity-key': 'ik'}});
      return new Response('{}', {status: 404});
    }) as typeof globalThis.fetch;
    const authorise = vi.fn(async () => {
      const token = `t${++s.issued}`;
      s.sessions.add(token);
      return {token, expiresAt: Date.now() / 1000 + 3600};
    });
    return {s, fetch, authorise, restart: () => ((s.up = true), s.sessions.clear())};
  }

  async function drawing() {
    const srv = server();
    const clock = fakeClock();
    const scheduler = fakeScheduler();
    const store = createStore({viewerUrl: 'http://viewer', authorise: srv.authorise, clientOptions: {sessionUrl: '', fetch: srv.fetch}, clock, scheduler, prefetch: false, replica: {revalidateAfterMs: Infinity}});
    await clock.advance(1);
    store.setView(camera({xMin: 0, xMax: 1000, yMin: 0, yMax: 1000}, [0, 0, 1000, 1000], 400, 400));
    await clock.advance(600);
    scheduler.flush();
    await settle();
    return {srv, clock, scheduler, store};
  }

  it('is reported, asked again with a growing wait, and recovered from with a new token', async () => {
    const {srv, clock, store} = await drawing();
    expect(store.get('status').unreachable).toBe(false);
    expect(srv.authorise).toHaveBeenCalledTimes(1);

    srv.s.up = false;
    store.refresh();
    await clock.advance(10);
    expect(store.get('status').unreachable).toBe(true);
    // Asked again after 1 s, 2 s and 4 s, and not between.
    await clock.advance(7_100);
    srv.s.up = true;
    srv.s.sessions.clear();
    srv.s.asked.length = 0;
    await clock.advance(7_000);
    expect(srv.s.asked).toEqual([]);
    await clock.advance(1_000);
    await settle();
    expect(store.get('status').unreachable).toBe(false);
    expect(srv.authorise).toHaveBeenCalledTimes(2);
    // What is drawn is asked for again, under the new token, and the server answers it.
    await clock.advance(600);
    await settle();
    expect(srv.s.asked.filter((r) => r !== '/v1/meta').length).toBeGreaterThan(0);
    expect(store.get('status').status).not.toBe('refused');
    store.dispose();
  });

  it('a 401 on a token that worked asks authorise for another, once', async () => {
    const {srv, clock, store} = await drawing();
    srv.restart();
    store.refresh();
    await clock.advance(1_000);
    await settle();
    expect(srv.authorise).toHaveBeenCalledTimes(2);
    expect(store.get('status').expired).toBe(false);
    // A new token the server refuses from the start is not asked again in a loop.
    srv.s.sessions.clear();
    srv.authorise.mockImplementation(async () => ({token: 'never', expiresAt: Date.now() / 1000 + 3600}));
    store.refresh();
    await clock.advance(1_000);
    await settle();
    store.refresh();
    await clock.advance(1_000);
    await settle();
    expect(srv.authorise.mock.calls.length).toBeLessThanOrEqual(4);
    store.dispose();
  });

  it('a store made while the server is down, whose token route fails with it, reads meta once it is back', async () => {
    const srv = server();
    srv.s.up = false;
    const authorise = vi.fn(async () => {
      if (!srv.s.up) throw new Error('token route answered 502');
      return srv.authorise();
    });
    const clock = fakeClock();
    const store = createStore({viewerUrl: 'http://viewer', authorise, clientOptions: {sessionUrl: '', fetch: srv.fetch}, clock, scheduler: fakeScheduler(), prefetch: false});
    await clock.advance(10);
    await settle();
    expect(store.get('status').unreachable).toBe(true);
    expect(store.get('meta')).toBeNull();
    srv.s.up = true;
    await clock.advance(4_000);
    await settle();
    expect(store.get('status').unreachable).toBe(false);
    expect(store.get('meta')).not.toBeNull();
    store.dispose();
  });
});
