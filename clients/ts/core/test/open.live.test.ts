import {afterAll, beforeAll, describe, expect, it, type TestContext} from 'vitest';
import {Control} from '../src/control.js';
import {createStore} from '../src/store.js';
import {start, type Served} from './served.js';

/**
 * A real `mosaica serve --open` (`served.ts`): a store given only the viewer's address, and a
 * control client given no credential, are served, and no request carries an `authorization`
 * header.
 */

let served: Served | string = 'the server has not started';

beforeAll(async () => {
  served = await start({open: true});
}, 120_000);

afterAll(() => {
  if (typeof served !== 'string') served.stop();
});

function live(ctx: TestContext): void {
  if (typeof served === 'string') ctx.skip(served);
}

/** `fetch`, recording whether each request carried an `authorization` header. */
function watching(): {fetch: typeof fetch; authorised: boolean[]} {
  const authorised: boolean[] = [];
  const watched: typeof fetch = (input, init) => {
    authorised.push(new Headers(init?.headers).has('authorization'));
    return fetch(input, init);
  };
  return {fetch: watched, authorised};
}

async function until(what: string, ok: () => boolean): Promise<void> {
  const deadline = Date.now() + 20_000;
  while (!ok()) {
    if (Date.now() > deadline) throw new Error(`timed out waiting for ${what}`);
    await new Promise((r) => setTimeout(r, 50));
  }
}

describe('an open server', () => {
  it('serves a store built from the viewer address alone', async (ctx) => {
    live(ctx);
    const {fetch, authorised} = watching();
    const store = createStore({viewerUrl: (served as Served).viewerUrl, prefetch: false, clientOptions: {sessionUrl: '', fetch}});
    try {
      await until('/v1/meta', () => store.get('meta') !== null || store.get('status').status === 'refused');
      expect(store.get('status').refusal).toBeNull();
      expect(store.get('meta')!.views.length).toBeGreaterThan(0);
      expect(authorised.length).toBeGreaterThan(0);
      expect(authorised.every((sent) => !sent)).toBe(true);
    } finally {
      store.dispose();
    }
  });

  it('takes a write from a control client with no credential', async (ctx) => {
    live(ctx);
    const {fetch, authorised} = watching();
    const control = new Control({controlUrl: (served as Served).controlUrl, fetch});
    const flushed = await control.flush();
    expect(flushed.status).toBe(202);
    const status = await control.status();
    expect(status.status).toBe(200);
    expect(authorised).toEqual([false, false]);
  });
});
