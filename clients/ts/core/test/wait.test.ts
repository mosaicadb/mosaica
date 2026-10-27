import {describe, expect, it} from 'vitest';
import {MosaicaClient, MosaicaError} from '../src/client.js';

/**
 * The wait a `429` asks for, as `MosaicaClient` reads it on every route: the `Retry-After` header,
 * then the body's `retry_after_s`, then 1 s, and never less than 1 s. `Control` reads it by the same
 * rule, which its own tests check through its waits.
 */

type Reply = {headers?: Record<string, string>; body?: unknown};

function clientAnswering(status: number, reply: Reply): MosaicaClient {
  return new MosaicaClient({
    viewerUrl: 'http://viewer',
    sessionUrl: 'http://session',
    fetch: (async () =>
      new Response(reply.body === undefined ? 'not json' : JSON.stringify(reply.body), {status, headers: reply.headers})) as typeof fetch
  });
}

const CASES: [string, Reply, number][] = [
  ['a header and a body', {headers: {'retry-after': '2'}, body: {error: 'backpressure', detail: 'shed', retry_after_s: 5}}, 2],
  ['a body alone', {body: {error: 'backpressure', detail: 'shed', retry_after_s: 3}}, 3],
  ['a header asking for 0 s', {headers: {'retry-after': '0'}, body: {error: 'backpressure', detail: 'shed'}}, 1],
  ['a body asking for 0.2 s', {body: {error: 'backpressure', detail: 'shed', retry_after_s: 0.2}}, 1],
  ['no wait named', {}, 1]
];

describe('the wait a 429 asks for', () => {
  for (const [name, reply, wait] of CASES) {
    it(`is ${wait} s from ${name}, on a refused route and on suggest`, async () => {
      const refused = await clientAnswering(429, reply).meta('tok').catch((error: unknown) => error);
      expect(refused).toBeInstanceOf(MosaicaError);
      expect((refused as MosaicaError).retryAfterS).toBe(wait);
      expect(await clientAnswering(429, reply).suggest('tok', 'archive', 'cs')).toMatchObject({status: 'shed', retryAfterS: wait});
    });
  }

  it('is null on a refusal that is not a 429 and names none', async () => {
    const refused = await clientAnswering(422, {body: {error: 'contract', detail: 'bad'}}).meta('tok').catch((error: unknown) => error);
    expect((refused as MosaicaError).retryAfterS).toBeNull();
  });
});
