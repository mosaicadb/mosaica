import {afterEach, describe, expect, it, vi} from 'vitest';
import {
  Control,
  MAX_BACKOFF,
  MIN_BACKOFF,
  RETRY_BUDGET_S,
  UNANSWERED,
  type Answer,
  type ChangeItem,
  type CompactionAnswered,
  type ControlStatus,
  type GrowRequest,
  type Progress,
  type PublishRequest,
  type StrictOptions,
  type WriteOptions
} from '../src/control.js';
import {headersOf} from './support.js';

/**
 * `Control` against a recording `fetch`: the request each method sends, the `429` retry, and that
 * every other status comes back as an answer. The live file checks the same routes against a server.
 */

type Sent = {method: string; url: string; headers: Record<string, string>; body: unknown; signal: AbortSignal | undefined};
type Reply = {status: number; body?: unknown; headers?: Record<string, string>};

/** Stubs `fetch` to record each request and answer the replies in turn, the last one for ever. */
function recording(...replies: Reply[]): Sent[] {
  const sent: Sent[] = [];
  vi.stubGlobal('fetch', async (url: string, init: RequestInit = {}) => {
    sent.push({method: init.method ?? 'GET', url, headers: headersOf(init), body: init.body, signal: init.signal ?? undefined});
    const reply = replies[Math.min(sent.length - 1, replies.length - 1)]!;
    return {
      status: reply.status,
      headers: new Headers(reply.headers ?? {}),
      text: async () => (reply.body === undefined ? '' : JSON.stringify(reply.body))
    } as unknown as Response;
  });
  return sent;
}

const control = new Control({controlUrl: 'http://control/', credential: 'op-cred'});
const rows = new Uint8Array([1, 2, 3, 4]);

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe('each route', () => {
  const JSON_ROUTE = {'content-type': 'application/json'};
  const PUBLISHED: PublishRequest = {
    level: 0,
    artifacts: [{key: 'c0', members: {mosaica_id: ['12', null], doi: [null, '10.1/x']}, content: [{values: ['c'], generated_from: {geonameid: [5]}}]}]
  };
  const GROWN: GrowRequest = {level: 0, artifacts: [{key: 'c0', rank: 0, members: {}, leaving: {mosaica_id: ['12']}}]};
  const CHANGES: ChangeItem[] = [
    {op: 'suppress', match: {doi: '10.1/x'}},
    {op: 'delete', match: {mosaica_id: '12', geonameid: 5}}
  ];
  const cases: {name: string; call: (c: Control) => Promise<Answer>; method: string; path: string; headers?: Record<string, string>; json?: unknown}[] = [
    {name: 'status', call: (c) => c.status(), method: 'GET', path: '/control/status'},
    {name: 'declareLayer', call: (c) => c.declareLayer({name: 'a/b'}), method: 'PUT', path: '/control/layers', headers: JSON_ROUTE, json: {name: 'a/b'}},
    {name: 'declareViewGroup', call: (c) => c.declareViewGroup('g', {extent: {x: [0, 1], y: [0, 1]}}), method: 'PUT', path: '/control/view_groups/g', headers: JSON_ROUTE, json: {extent: {x: [0, 1], y: [0, 1]}}},
    {name: 'declareAttribute', call: (c) => c.declareAttribute({name: 'note', type: 'keyword'}), method: 'PUT', path: '/control/attributes', headers: JSON_ROUTE, json: {name: 'note', type: 'keyword'}},
    {name: 'declareShards', call: (c) => c.declareShards({size: 4096}), method: 'PUT', path: '/control/shards', headers: JSON_ROUTE, json: {size: 4096}},
    {name: 'declareVocabulary', call: (c) => c.declareVocabulary('v', {width: 'u8'}), method: 'PUT', path: '/control/vocabularies/v', headers: JSON_ROUTE, json: {width: 'u8'}},
    {name: 'vocabularyValues', call: (c) => c.vocabularyValues('v', {values: [{key: 'k'}]}), method: 'PATCH', path: '/control/vocabularies/v/values', headers: JSON_ROUTE, json: {values: [{key: 'k'}]}},
    {name: 'declareView', call: (c) => c.declareView('plain', {extent: {x: [0, 1], y: [0, 1]}}), method: 'PUT', path: '/control/views/plain', headers: JSON_ROUTE, json: {extent: {x: [0, 1], y: [0, 1]}}},
    {name: 'createView', call: (c) => c.createView('g', 'k 1', {metadata: {}}), method: 'PUT', path: '/control/views/g/k%201', headers: JSON_ROUTE, json: {metadata: {}}},
    {name: 'publish', call: (c) => c.publish('clusters/kmeans', PUBLISHED), method: 'PUT', path: '/control/layers/clusters%2Fkmeans/artifacts', headers: JSON_ROUTE, json: PUBLISHED},
    {name: 'grow (JSON)', call: (c) => c.grow('clusters/kmeans', GROWN), method: 'PATCH', path: '/control/layers/clusters%2Fkmeans/artifacts', headers: JSON_ROUTE, json: GROWN},
    {name: 'changes', call: (c) => c.changes(CHANGES), method: 'POST', path: '/control/changes', headers: JSON_ROUTE, json: CHANGES},
    {name: 'dropLayer', call: (c) => c.dropLayer('a/b'), method: 'DELETE', path: '/control/layers/a%2Fb'},
    {name: 'dropLayer waiting', call: (c) => c.dropLayer('a', {wait: true}), method: 'DELETE', path: '/control/layers/a?wait=visible'},
    {name: 'dropView', call: (c) => c.dropView('g', 'k'), method: 'DELETE', path: '/control/views/g/k'},
    {name: 'dropView waiting', call: (c) => c.dropView('g', 'k', {wait: true}), method: 'DELETE', path: '/control/views/g/k?wait=visible'},
    {name: 'compact', call: (c) => c.compact(), method: 'POST', path: '/control/compact'},
    {name: 'flush', call: (c) => c.flush(), method: 'POST', path: '/control/flush'},
    {name: 'flush waiting', call: (c) => c.flush({wait: true}), method: 'POST', path: '/control/flush?wait=visible'}
  ];

  for (const k of cases) {
    it(`${k.name} sends ${k.method} ${k.path} with the operator credential`, async () => {
      const sent = recording({status: 200, body: {}});
      const answer = await k.call(control);
      expect(answer.ok).toBe(true);
      expect(sent).toHaveLength(1);
      const [request] = sent;
      expect(request!.method).toBe(k.method);
      expect(request!.url).toBe(`http://control${k.path}`);
      expect(request!.headers).toMatchObject({authorization: 'Bearer op-cred', ...k.headers});
      if (k.json !== undefined) expect(JSON.parse(request!.body as string)).toEqual(k.json);
    });
  }

  const writes: {name: string; call: (c: Control, o: WriteOptions) => Promise<Answer>; path: string}[] = [
    {name: 'ingest', call: (c, o) => c.ingest(rows, o), path: '/control/ingest'},
    {name: 'declareLayer', call: (c, o) => c.declareLayer({}, o), path: '/control/layers'},
    {name: 'declareViewGroup', call: (c, o) => c.declareViewGroup('g', {}, o), path: '/control/view_groups/g'},
    {name: 'declareAttribute', call: (c, o) => c.declareAttribute({}, o), path: '/control/attributes'},
    {name: 'declareShards', call: (c, o) => c.declareShards({}, o), path: '/control/shards'},
    {name: 'declareVocabulary', call: (c, o) => c.declareVocabulary('v', {}, o), path: '/control/vocabularies/v'},
    {name: 'vocabularyValues', call: (c, o) => c.vocabularyValues('v', {}, o), path: '/control/vocabularies/v/values'},
    {name: 'declareView', call: (c, o) => c.declareView('p', {}, o), path: '/control/views/p'},
    {name: 'createView', call: (c, o) => c.createView('g', 'k', {}, o), path: '/control/views/g/k'},
    {name: 'publish', call: (c, o) => c.publish('l', {artifacts: []}, o), path: '/control/layers/l/artifacts'},
    {name: 'grow', call: (c, o) => c.grow('l', rows, o), path: '/control/layers/l/artifacts'},
    {name: 'changes', call: (c, o) => c.changes([], o), path: '/control/changes'},
    {name: 'dropLayer', call: (c, o) => c.dropLayer('l', o), path: '/control/layers/l'},
    {name: 'dropView', call: (c, o) => c.dropView('g', 'k', o), path: '/control/views/g/k'},
    {name: 'flush', call: (c, o) => c.flush(o), path: '/control/flush'}
  ];

  for (const w of writes) {
    it(`${w.name} asks for wait=visible only when told to, and hands fetch the caller's signal`, async () => {
      const sent = recording({status: 200, body: {}});
      const signal = new AbortController().signal;
      await w.call(control, {});
      await w.call(control, {wait: true, signal});
      expect(sent.map((s) => s.url)).toEqual([`http://control${w.path}`, `http://control${w.path}?wait=visible`]);
      expect(sent.map((s) => s.signal)).toEqual([undefined, signal]);
    });
  }

  const naming: {name: string; call: (c: Control, o: StrictOptions) => Promise<Answer>; path: string}[] = [
    {name: 'ingest', call: (c, o) => c.ingest(rows, o), path: '/control/ingest'},
    {name: 'changes', call: (c, o) => c.changes([], o), path: '/control/changes'},
    {name: 'publish', call: (c, o) => c.publish('l', {artifacts: []}, o), path: '/control/layers/l/artifacts'},
    {name: 'grow', call: (c, o) => c.grow('l', rows, o), path: '/control/layers/l/artifacts'}
  ];

  for (const w of naming) {
    it(`${w.name} sends strict only when given, as given`, async () => {
      const sent = recording({status: 200, body: {}});
      await w.call(control, {});
      await w.call(control, {strict: true});
      await w.call(control, {strict: false, wait: true});
      expect(sent.map((s) => s.url)).toEqual([
        `http://control${w.path}`,
        `http://control${w.path}?strict=true`,
        `http://control${w.path}?wait=visible&strict=false`
      ]);
    });
  }

  it('sends every route through the host’s fetch, with its headers beside the route’s own', async () => {
    vi.stubGlobal('fetch', async () => {
      throw new Error('the global fetch was used');
    });
    const sent: Record<string, string>[] = [];
    const hosted = async (_url: string | URL | Request, init?: RequestInit) => {
      sent.push(headersOf(init));
      return new Response('{}', {status: 200});
    };
    const c = new Control({controlUrl: 'http://control', credential: 'op-cred', fetch: hosted as typeof fetch, headers: {'X-Host': 'script', Authorization: 'Bearer host'}});
    const calls = [...cases.map((k) => k.call), ...writes.map((w) => (c: Control) => w.call(c, {})), (c: Control) => c.grow('l', rows)];
    for (const call of calls) {
      sent.length = 0;
      expect((await call(c)).ok).toBe(true);
      expect(sent).toHaveLength(1);
      expect(sent[0]).toMatchObject({'x-host': 'script', authorization: 'Bearer op-cred'});
    }
  });

  it('grow sends bytes as an Arrow stream', async () => {
    const sent = recording({status: 200, body: {}});
    await control.grow('l', rows);
    expect(sent[0]!.headers['content-type']).toBe('application/vnd.apache.arrow.stream');
    expect(sent[0]!.body).toBe(rows);
  });

  for (const route of ['ingest'] as const) {
    it(`${route} sends the bytes as given, under the caller's batch id and view`, async () => {
      const sent = recording({status: 200, body: {created: 4}});
      const answer = await control[route](rows, {batch: 'page-0', view: 's0'});
      expect(sent[0]).toMatchObject({method: 'POST', url: `http://control/control/${route}`, body: rows});
      expect(sent[0]!.headers).toEqual({
        authorization: 'Bearer op-cred',
        'content-type': 'application/vnd.apache.arrow.stream',
        'x-mosaica-batch-id': 'page-0',
        'x-mosaica-view': 's0'
      });
      expect(answer.batch).toBe('page-0');
      expect(answer.body).toEqual({created: 4});
    });

    it(`${route} names no view unless given one, and makes a fresh batch id for each call`, async () => {
      const sent = recording({status: 200, body: {}});
      const first = await control[route](rows);
      const second = await control[route](rows);
      expect(sent[0]!.headers).not.toHaveProperty('x-mosaica-view');
      expect(first.batch).not.toBe(second.batch);
      expect(sent.map((s) => s.headers['x-mosaica-batch-id'])).toEqual([first.batch, second.batch]);
    });
  }
});

describe('an answer', () => {
  it('after a 429, waits its Retry-After and resends the same batch id and bytes', async () => {
    vi.useFakeTimers();
    const sent = recording({status: 429, headers: {'retry-after': '2'}}, {status: 429, headers: {'retry-after': '2'}}, {status: 200, body: {created: 4}});
    const pending = control.ingest(rows, {view: 's0'});
    await vi.advanceTimersByTimeAsync(1_999);
    expect(sent).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(sent).toHaveLength(2);
    await vi.advanceTimersByTimeAsync(2_000);
    const answer = await pending;
    expect(answer).toMatchObject({status: 200, ok: true, attempts: 3, body: {created: 4}});
    expect(sent.map((s) => s.body)).toEqual([rows, rows, rows]);
    expect(new Set(sent.map((s) => s.headers['x-mosaica-batch-id']))).toEqual(new Set([answer.batch]));
  });

  it('takes the body’s retry_after_s where a 429 has no header', async () => {
    vi.useFakeTimers();
    const sent = recording({status: 429, body: {error: 'backpressure', retry_after_s: 3}}, {status: 200, body: {}});
    const pending = control.flush();
    await vi.advanceTimersByTimeAsync(2_999);
    expect(sent).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(1);
    expect((await pending).attempts).toBe(2);
  });

  it('is the 429 whose wait would take the waits past the budget', async () => {
    vi.useFakeTimers();
    const sent = recording({status: 429, headers: {'retry-after': '7'}, body: {error: 'backpressure'}});
    const started = Date.now();
    const pending = control.ingest(rows);
    await vi.runAllTimersAsync();
    const answer = await pending;
    const waits = Math.floor(RETRY_BUDGET_S / 7);
    expect(answer).toMatchObject({status: 429, ok: false, attempts: waits + 1, body: {error: 'backpressure'}});
    expect(sent).toHaveLength(waits + 1);
    expect(Date.now() - started).toBe(waits * 7 * 1000);
    expect(new Set(sent.map((s) => s.headers['x-mosaica-batch-id'])).size).toBe(1);
  });

  for (const [name, reply, waited] of [
    ['a header asking for 0 s', {headers: {'retry-after': '0'}}, MIN_BACKOFF],
    ['a header asking for 100 s', {headers: {'retry-after': '100'}}, MAX_BACKOFF],
    ['a header and a body', {headers: {'retry-after': '2'}, body: {retry_after_s: 5}}, 2],
    ['a body asking for 0.2 s', {body: {retry_after_s: 0.2}}, MIN_BACKOFF],
    ['no wait named', {}, MIN_BACKOFF]
  ] as const) {
    it(`after a 429 with ${name}, waits ${waited} s`, async () => {
      vi.useFakeTimers();
      const sent = recording({status: 429, ...reply}, {status: 200, body: {}});
      const pending = control.status();
      await vi.advanceTimersByTimeAsync(waited * 1000 - 1);
      expect(sent).toHaveLength(1);
      await vi.advanceTimersByTimeAsync(1);
      expect((await pending).attempts).toBe(2);
    });
  }

  it('ends a 429 wait when the signal aborts, rejecting with its reason and sending nothing more', async () => {
    vi.useFakeTimers();
    const sent = recording({status: 429, headers: {'retry-after': '5'}});
    const controller = new AbortController();
    const pending = control.flush({signal: controller.signal});
    const outcome = pending.then(
      () => 'resolved',
      (reason: unknown) => reason
    );
    await vi.advanceTimersByTimeAsync(1_000);
    const reason = new Error('stop');
    controller.abort(reason);
    expect(await outcome).toBe(reason);
    await vi.advanceTimersByTimeAsync(10_000);
    expect(sent).toHaveLength(1);
  });

  it('rejects with the signal’s reason when it aborts a request in flight', async () => {
    vi.stubGlobal('fetch', (_url: string, init: RequestInit) =>
      new Promise((_resolve, reject) => init.signal!.addEventListener('abort', () => reject(new DOMException('aborted', 'AbortError'))))
    );
    const controller = new AbortController();
    const pending = control.status({signal: controller.signal});
    const reason = new Error('stop');
    controller.abort(reason);
    await expect(pending).rejects.toBe(reason);
  });

  it('is any other refusal as the server sent it, sent once', async () => {
    const sent = recording({status: 409, body: {error: 'conflict', detail: 'held'}});
    const answer = await control.publish('l', {level: 0, artifacts: []});
    expect(answer).toMatchObject({status: 409, ok: false, attempts: 1, body: {error: 'conflict', detail: 'held'}});
    expect(sent).toHaveLength(1);
  });

  it('carries status 0 where no server answered', async () => {
    vi.stubGlobal('fetch', async () => {
      throw new TypeError('fetch failed');
    });
    const answer = await control.changes([{op: 'delete', match: {mosaica_id: '5'}}]);
    expect(answer).toMatchObject({status: UNANSWERED, ok: false, attempts: 1});
  });

  it('sends an ingest that reached no server again, the same bytes under the same batch id', async () => {
    vi.useFakeTimers();
    const sent: {batch: string | null; body: unknown}[] = [];
    vi.stubGlobal('fetch', async (_url: string, init: RequestInit) => {
      sent.push({batch: new Headers(init.headers).get('x-mosaica-batch-id'), body: init.body});
      if (sent.length === 1) throw new TypeError('fetch failed');
      return {status: 200, headers: new Headers(), text: async () => '{"created": 1}'} as unknown as Response;
    });
    const pending = control.ingest(new Uint8Array([1, 2, 3]));
    await vi.advanceTimersByTimeAsync(1_000);
    const answer = await pending;
    expect(answer).toMatchObject({status: 200, ok: true, attempts: 2});
    expect(sent).toHaveLength(2);
    expect(sent[1]).toEqual(sent[0]);
  });

  it('answers an ingest no server answered twice with status 0, sent once more and no more', async () => {
    vi.useFakeTimers();
    const sent: {batch: string | null; body: unknown}[] = [];
    vi.stubGlobal('fetch', async (_url: string, init: RequestInit) => {
      sent.push({batch: new Headers(init.headers).get('x-mosaica-batch-id'), body: init.body});
      throw new TypeError('fetch failed');
    });
    const pending = control.ingest(new Uint8Array([1, 2, 3]));
    await vi.advanceTimersByTimeAsync(10_000);
    const answer = await pending;
    expect(answer).toMatchObject({status: UNANSWERED, ok: false, attempts: 2});
    expect(sent).toHaveLength(2);
    expect(sent[1]).toEqual(sent[0]);
  });

  it('takes nothing from the 429 budget for the resend of an ingest no server answered', async () => {
    vi.useFakeTimers();
    let calls = 0;
    vi.stubGlobal('fetch', async () => {
      calls += 1;
      if (calls === 1) throw new TypeError('fetch failed');
      return {
        status: 429,
        headers: new Headers({'retry-after': '7'}),
        text: async () => '{"error": "backpressure"}'
      } as unknown as Response;
    });
    const started = Date.now();
    const pending = control.ingest(rows);
    await vi.runAllTimersAsync();
    const answer = await pending;
    const waits = Math.floor(RETRY_BUDGET_S / 7);
    expect(answer).toMatchObject({status: 429, attempts: waits + 2});
    expect(Date.now() - started).toBe(MIN_BACKOFF * 1000 + waits * 7 * 1000);
  });

  it('keeps a non-object JSON body under `value`, and a non-JSON body only as text', async () => {
    recording({status: 200, body: [1, 2]});
    expect((await control.status()).body).toEqual({value: [1, 2]});
    vi.stubGlobal('fetch', async () => ({status: 502, headers: new Headers(), text: async () => 'bad gateway'}) as unknown as Response);
    expect(await control.status()).toMatchObject({status: 502, body: {}, text: 'bad gateway'});
  });

  it('limits is the status body’s limits block', async () => {
    recording({status: 200, body: {limits: {changes: {max_changes_per_request: 10_000}}}});
    expect(await control.limits()).toEqual({changes: {max_changes_per_request: 10_000}});
  });
});

describe('compact with wait', () => {
  const RUNNING: Progress = {
    job: 'compaction',
    state: 'running',
    phase: {index: 2, of: 4, name: 'Indexing records'},
    step: 'postings',
    context: 'shard 0',
    done: 1200,
    total: 4000,
    unit: 'terms',
    elapsed_s: 3.5,
    step_elapsed_s: 1.25,
    rss_bytes: 1 << 30
  };
  const FINISHED: Progress = {
    job: 'compaction',
    state: 'finished',
    phase: {index: 4, of: 4, name: 'Publishing'},
    step: 'reclaim',
    context: 'shard 0',
    elapsed_s: 9,
    step_elapsed_s: 0.5,
    rss_bytes: 1 << 30
  };
  const FAILED: Progress = {...RUNNING, state: 'failed', phase: {index: 3, of: 4, name: 'Verifying'}, step: 'digests', unit: 'bytes'};
  const FLUSHING: Progress = {job: 'flush', state: 'running', phase: {index: 1, of: 1, name: 'Flushing'}, step: 'promote', elapsed_s: 0.2, step_elapsed_s: 0.2, rss_bytes: 1 << 30};

  const answered = (first: number, last: number, outcome: CompactionAnswered['outcome'], gate: string | null = null, detail: string | null = null): CompactionAnswered => ({
    first,
    last,
    outcome,
    gate,
    detail
  });

  /** A `/control/status` body with the given compaction requests, jobs and last record, and fields the client does not read. */
  const status = (requests: {accepted: number; answered: number; last_answer: CompactionAnswered | null}, jobs: Progress[] = [], lastJob: Progress | null = null): {status: number; body: ControlStatus} => ({
    status: 200,
    body: {publication: 12, jobs, compaction: {folds: 3, last_job: lastJob, requests: {...requests, last_refusal: null}}}
  });

  const ACCEPTED = {status: 202, body: {request: 5}};

  it('without wait sends one request and answers it as before', async () => {
    const sent = recording(ACCEPTED, status({accepted: 5, answered: 5, last_answer: answered(5, 5, 'compacted')}));
    const seen: Progress[] = [];
    const answer = await control.compact({onProgress: (record) => seen.push(record)});
    expect(sent.map((s) => `${s.method} ${s.url}`)).toEqual(['POST http://control/control/compact']);
    expect(answer).toMatchObject({status: 202, ok: true, body: {request: 5}});
    expect(answer).not.toHaveProperty('waited');
    expect(seen).toEqual([]);
  });

  it('reads the status about once a second until the request is answered, passing the running record and then last_job as served', async () => {
    vi.useFakeTimers();
    const served = answered(4, 5, 'compacted');
    const sent = recording(
      ACCEPTED,
      status({accepted: 5, answered: 3, last_answer: answered(3, 3, 'nothing_to_do')}, [FLUSHING, RUNNING]),
      status({accepted: 5, answered: 5, last_answer: served}, [], FINISHED)
    );
    const seen: Progress[] = [];
    const pending = control.compact({wait: true, onProgress: (record) => seen.push(record)});
    await vi.advanceTimersByTimeAsync(999);
    expect(sent.map((s) => `${s.method} ${s.url}`)).toEqual(['POST http://control/control/compact', 'GET http://control/control/status']);
    await vi.advanceTimersByTimeAsync(1);
    const answer = await pending;
    expect(sent).toHaveLength(3);
    expect(sent[2]!.headers).toMatchObject({authorization: 'Bearer op-cred'});
    expect(answer).toMatchObject({status: 202, ok: true, body: {request: 5}});
    expect(answer.waited).toStrictEqual({ok: true, ended: 'answered', answer: served, last_job: FINISHED});
    expect(seen).toStrictEqual([RUNNING, FINISHED]);
  });

  it('answers nothing_to_do as succeeded, without the last_job of an earlier compaction', async () => {
    const served = answered(5, 5, 'nothing_to_do');
    recording(ACCEPTED, status({accepted: 5, answered: 5, last_answer: served}, [], FINISHED));
    const seen: Progress[] = [];
    const answer = await control.compact({wait: true, onProgress: (record) => seen.push(record)});
    expect(answer.waited).toStrictEqual({ok: true, ended: 'answered', answer: served});
    expect(seen).toEqual([]);
  });

  it('answers a refusal as the server served it, gate and detail included', async () => {
    const served = answered(5, 6, 'refused', 'insufficient_disc', 'the disc holding the bundle has too little free space');
    recording(ACCEPTED, status({accepted: 6, answered: 6, last_answer: served}, [], FINISHED));
    const answer = await control.compact({wait: true});
    expect(answer.ok).toBe(true);
    expect(answer.waited).toStrictEqual({ok: false, ended: 'answered', answer: served});
  });

  it('answers a failure with the failed record as served, which onProgress receives last', async () => {
    vi.useFakeTimers();
    const served = answered(5, 5, 'failed');
    recording(ACCEPTED, status({accepted: 5, answered: 4, last_answer: null}, [RUNNING]), status({accepted: 5, answered: 5, last_answer: served}, [], FAILED));
    const seen: Progress[] = [];
    const pending = control.compact({wait: true, onProgress: (record) => seen.push(record)});
    await vi.runAllTimersAsync();
    const answer = await pending;
    expect(answer.waited).toStrictEqual({ok: false, ended: 'answered', answer: served, last_job: FAILED});
    expect(seen).toStrictEqual([RUNNING, FAILED]);
  });

  it('ends as lost where the server has numbered fewer requests than this one', async () => {
    const sent = recording(ACCEPTED, status({accepted: 2, answered: 2, last_answer: answered(1, 2, 'compacted')}));
    const answer = await control.compact({wait: true});
    expect(answer.waited).toStrictEqual({ok: false, ended: 'lost', accepted: 2});
    expect(sent).toHaveLength(2);
  });

  it('ends as replaced where a later answer has replaced the request’s outcome', async () => {
    const later = answered(6, 7, 'compacted');
    recording(ACCEPTED, status({accepted: 7, answered: 7, last_answer: later}, [], FINISHED));
    expect((await control.compact({wait: true})).waited).toStrictEqual({ok: false, ended: 'replaced', answer: later});
  });

  it('ends as replaced where the request is answered and no answer is served', async () => {
    recording(ACCEPTED, status({accepted: 5, answered: 5, last_answer: null}));
    expect((await control.compact({wait: true})).waited).toStrictEqual({ok: false, ended: 'replaced', answer: null});
  });

  it('ends as unread with the status answer where reading the status is refused', async () => {
    const sent = recording(ACCEPTED, {status: 403, body: {error: 'forbidden', detail: 'admin'}});
    const answer = await control.compact({wait: true});
    expect(answer).toMatchObject({status: 202, ok: true, waited: {ok: false, ended: 'unread', status: {status: 403, ok: false}}});
    expect(sent).toHaveLength(2);
  });

  it('does not wait for a request the server refused', async () => {
    const sent = recording({status: 503, body: {error: 'not-ready', detail: 'the write side is not running'}});
    const answer = await control.compact({wait: true});
    expect(answer).toMatchObject({status: 503, ok: false});
    expect(answer).not.toHaveProperty('waited');
    expect(sent).toHaveLength(1);
  });

  it('rejects with the signal’s reason when it aborts between two readings, and reads no more', async () => {
    vi.useFakeTimers();
    const sent = recording(ACCEPTED, status({accepted: 5, answered: 4, last_answer: null}, [RUNNING]));
    const controller = new AbortController();
    const outcome = control.compact({wait: true, signal: controller.signal}).then(
      () => 'resolved',
      (reason: unknown) => reason
    );
    await vi.advanceTimersByTimeAsync(2_500);
    expect(sent).toHaveLength(4);
    const reason = new Error('stop');
    controller.abort(reason);
    expect(await outcome).toBe(reason);
    await vi.advanceTimersByTimeAsync(10_000);
    expect(sent).toHaveLength(4);
    expect(sent.every((s) => s.signal === controller.signal)).toBe(true);
  });
});
