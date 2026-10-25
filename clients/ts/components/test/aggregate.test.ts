import {describe, expect, it} from 'vitest';
import {countsByKey, listedGroups} from '../src/aggregate.js';
import {aggregateEntry} from './fake-store.js';

/**
 * The listed groups of an aggregate table. A field whose vocabulary holds no title has no `title`
 * column, and every group then reads a null title; a vocabulary with titles reads each group's own.
 */
describe('listedGroups', () => {
  it('reads every title as null from a table with no title column', () => {
    const entry = aggregateEntry([{rows: [{key: 'hep', count: 6}, {key: 'astro', count: 4}, {group: 'rest', count: 2}]}]);
    const table = entry.result!.tables[0]!;
    expect(table.rows.getChild('title')).toBeNull();
    expect(listedGroups(table)).toEqual([
      {key: 'hep', title: null, count: 6, slot: null},
      {key: 'astro', title: null, count: 4, slot: null}
    ]);
    expect(countsByKey(entry)).toEqual(new Map([['hep', 6], ['astro', 4]]));
  });

  it('reads each title, and null where a value has none, from a table with the column', () => {
    const entry = aggregateEntry([{rows: [{key: 'hep', title: 'High energy physics', count: 6}, {key: 'astro', title: null, count: 4}, {group: 'none', count: 1}]}]);
    const table = entry.result!.tables[0]!;
    expect(table.rows.getChild('title')).not.toBeNull();
    expect(listedGroups(table)).toEqual([
      {key: 'hep', title: 'High energy physics', count: 6, slot: null},
      {key: 'astro', title: null, count: 4, slot: null}
    ]);
  });
});
