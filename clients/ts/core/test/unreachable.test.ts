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
    const s = {up: true, sessions: new Set<string>(), issued: 0, asked: [] as string[], slow: null as typeof globalThis.fetch | null};
    const answer = (async (url: string, init?: RequestInit) => {
      if (!s.up) throw new TypeError('fetch failed');
      const route = new URL(url).pathname;
      s.asked.push(route);
      const token = headersOf(init)['authorization']?.slice(7) ?? '';
      if (!s.sessions.has(token)) return new Response(JSON.stringify({error: 'bad-credential', detail: 'no such session'}), {status: 401, headers: {'content-type': 'application/json'}});
      if (route === '/v1/meta') return new Response(META, {headers: {'content-type': 'application/json'}});
      if (route === '/v1/viewport') return new Response(VIEWPORT.slice(), {headers: {etag: '"ck"', 'x-mosaica-identity-key': 'ik'}});
      return new Response('{}', {status: 404});
    }) as typeof globalThis.fetch;
    // A test may put another fetch in front of the server's answers.
    const fetch = ((url: string, init?: RequestInit) => (s.slow ? s.slow(url, init) : answer(url, init))) as typeof globalThis.fetch;
    const authorise = vi.fn(async () => {
      const token = `t${++s.issued}`;
      s.sessions.add(token);
      return {token, expiresAt: Date.now() / 1000 + 3600};
    });
    return {s, fetch, answer, authorise, restart: () => ((s.up = true), s.sessions.clear())};
  }

  async function drawing() {
    const srv = server();
    const clock = fakeClock();
    const scheduler = fakeScheduler();
    const store = createStore({viewerUrl: 'http://viewer', authorise: srv.authorise, clientOptions: {fetch: srv.fetch}, clock, scheduler, prefetch: false, replica: {revalidateAfterMs: Infinity}});
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

  it('a store made while the server is down reads meta once it is back, with the token it was given', async () => {
    const srv = server();
    srv.s.up = false;
    const clock = fakeClock();
    const store = createStore({viewerUrl: 'http://viewer', authorise: srv.authorise, clientOptions: {fetch: srv.fetch}, clock, scheduler: fakeScheduler(), prefetch: false});
    await clock.advance(10);
    await settle();
    expect(store.get('status').unreachable).toBe(true);
    expect(store.get('meta')).toBeNull();
    srv.s.up = true;
    await clock.advance(4_000);
    await settle();
    expect(store.get('status').unreachable).toBe(false);
    expect(store.get('meta')).not.toBeNull();
    expect(srv.authorise).toHaveBeenCalledTimes(1);
    store.dispose();
  });

  it('a supplier that refuses is a refusal: asked once, and the server is not called unreachable', async () => {
    const srv = server();
    const authorise = vi.fn(async (): Promise<{token: string; expiresAt: number}> => {
      throw new Error('principal "eve" is not known');
    });
    const clock = fakeClock();
    const store = createStore({viewerUrl: 'http://viewer', authorise, clientOptions: {fetch: srv.fetch}, clock, scheduler: fakeScheduler(), prefetch: false});
    for (let i = 0; i < 120; i++) {
      await clock.advance(1_000);
      await settle();
    }
    expect(authorise).toHaveBeenCalledTimes(1);
    expect(store.get('status').unreachable).toBe(false);
    expect(store.get('status').status).toBe('refused');
    store.dispose();
  });

  it('one route timing out at a proxy is not an outage, and costs no new session', async () => {
    const {srv, clock, scheduler, store} = await drawing();
    const fetch = srv.answer;
    srv.s.asked.length = 0;
    const slow = (async (url: string, init?: RequestInit) => (new URL(url).pathname === '/v1/viewport' ? new Response('gateway timeout', {status: 504}) : fetch(url, init))) as typeof globalThis.fetch;
    // The store's client reads the fetch it was given, so the server's own is swapped.
    srv.s.slow = slow;
    store.setView(camera({xMin: 0, xMax: 1000, yMin: 0, yMax: 1000}, [0, 0, 250, 250], 400, 400));
    for (let i = 0; i < 60; i++) {
      await clock.advance(1_000);
      scheduler.flush();
      await settle();
    }
    expect(srv.authorise).toHaveBeenCalledTimes(1);
    expect(store.get('status').unreachable).toBe(false);
    expect(srv.s.asked.filter((r) => r === '/v1/viewport').length).toBeLessThan(5);
    store.dispose();
  });

  it('a server error with no Mosaica body is published with its status apart from the server’s own refusals, and a refresh recovers', async () => {
    const {srv, clock, scheduler, store} = await drawing();
    const fetch = srv.answer;
    const failing = (answer: () => Response) =>
      (async (url: string, init?: RequestInit) => (new URL(url).pathname === '/v1/viewport' ? answer() : fetch(url, init))) as typeof globalThis.fetch;
    // A box not yet drawn, so the store asks the server; with none, the refresh Retry makes.
    const ask = async (box?: [number, number, number, number]) => {
      if (box) store.setView(camera({xMin: 0, xMax: 1000, yMin: 0, yMax: 1000}, box, 400, 400));
      else store.refresh();
      for (let i = 0; i < 5; i++) {
        await clock.advance(1_000);
        scheduler.flush();
        await settle();
      }
      return store.get('status');
    };

    srv.s.slow = failing(() => new Response(null, {status: 504}));
    expect(await ask([0, 0, 250, 250])).toMatchObject({status: 'refused', refusal: {code: 'http-error', status: 504}, unreachable: false, expired: false});

    srv.s.slow = failing(() => new Response(JSON.stringify({error: 'fail-closed', detail: 'postings unreadable'}), {status: 500, headers: {'content-type': 'application/json'}}));
    expect(await ask([250, 250, 500, 500])).toMatchObject({status: 'refused', refusal: {code: 'fail-closed', detail: 'postings unreadable', status: 500}});

    srv.s.slow = null;
    expect(await ask()).toMatchObject({status: 'shown', refusal: null});
    expect(srv.authorise).toHaveBeenCalledTimes(1);
    store.dispose();
  });

  it('a route a proxy cannot reach while others answer waits longer each time, and costs no new session', async () => {
    const {srv, clock, scheduler, store} = await drawing();
    const fetch = srv.answer;
    srv.s.asked.length = 0;
    srv.s.slow = (async (url: string, init?: RequestInit) => (new URL(url).pathname === '/v1/viewport' ? new Response('bad gateway', {status: 502}) : fetch(url, init))) as typeof globalThis.fetch;
    store.setView(camera({xMin: 0, xMax: 1000, yMin: 0, yMax: 1000}, [0, 0, 250, 250], 400, 400));
    for (let i = 0; i < 300; i++) {
      await clock.advance(1_000);
      scheduler.flush();
      await settle();
    }
    expect(srv.authorise).toHaveBeenCalledTimes(1);
    expect(srv.s.asked.filter((r) => r === '/v1/viewport').length).toBeLessThan(20);
    store.dispose();
  });

  it('a token supplied after a session ended is checked for whose it is', async () => {
    const {srv, clock, store} = await drawing();
    const bodies: unknown[] = [];
    const fetch = srv.answer;
    srv.s.slow = (async (url: string, init?: RequestInit) => {
      if (new URL(url).pathname === '/v1/viewport' && typeof init?.body === 'string') bodies.push(JSON.parse(init.body));
      return fetch(url, init);
    }) as typeof globalThis.fetch;
    srv.restart();
    store.refresh();
    await clock.advance(1_000);
    await settle();
    expect(srv.authorise).toHaveBeenCalledTimes(2);
    // The identity check: one tile at depth 0, counts only, with no layer.
    expect(bodies.some((b) => (b as {k?: number; zoom?: number}).k === 0 && (b as {zoom?: number}).zoom === 0)).toBe(true);
    store.dispose();
  });
});
