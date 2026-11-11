import {describe, expect, it} from 'vitest';
import type {LevelsInView} from '../src/artifactChannel.js';
import {clustersInView, colourLayers, drawableLayers, follows, isFilterLayer, layerClosure, layerEntries, levelInView, levelOfLayer} from '../src/layers.js';
import type {Layer} from '../src/types.js';
import {layer as declared} from './support.js';

const layer = (name: string, depsOn: string[] = []): Layer => declared(name, {depsOn});

/**
 * A clustering's labels are a second layer that `depends_on` it: the picker
 * offers the clustering with its labels as one entry, and the request names both.
 */
describe('the layer closure', () => {
  const layers = [layer('clusters'), layer('labels', ['clusters']), layer('districts'), layer('routes', ['districts', 'clusters'])];

  it('names a layer with its dependents and their dependencies, in meta order', () => {
    expect(layerClosure(layers, ['clusters'])).toEqual(['clusters', 'labels', 'districts', 'routes']);
    expect(layerClosure(layers, ['districts'])).toEqual(['clusters', 'labels', 'districts', 'routes']);
    expect(layerClosure([layer('a'), layer('b', ['a']), layer('c')], ['a'])).toEqual(['a', 'b']);
  });

  it('keeps a name meta does not list, and names nothing twice', () => {
    expect(layerClosure(layers, ['nope', 'nope'])).toEqual(['nope']);
    expect(layerClosure([], [])).toEqual([]);
  });

  it('offers one entry per root, each carrying its closure and never a count', () => {
    const entries = layerEntries([layer('a'), layer('b', ['a']), layer('c')]);
    expect(entries.map((e) => e.root.name)).toEqual(['a', 'c']);
    expect(entries[0]!.closure).toEqual(['a', 'b']);
    expect(Object.keys(entries[0]!)).toEqual(['root', 'closure']);
  });

  it('offers a dependent whose dependency this principal does not reach as a root of what they were given', () => {
    const entries = layerEntries([layer('labels', ['clusters'])]);
    expect(entries.map((e) => e.root.name)).toEqual(['labels']);
  });
});

describe('a filter layer', () => {
  it('is a layer declaring no computed geometry that depends on nothing', () => {
    expect(isFilterLayer(layer('descriptors'))).toBe(true);
    expect(isFilterLayer({...layer('clusters'), computedContent: ['centroid']})).toBe(false);
  });

  it('is never a labels layer, which declares no geometry and is drawn at the artifact it names', () => {
    const clusters = {...layer('clusters'), computedContent: ['centroid']};
    const labels = layer('labels', ['clusters']);
    expect(isFilterLayer(labels)).toBe(false);
    expect(drawableLayers([clusters, labels, layer('descriptors')]).map((l) => l.name)).toEqual(['clusters', 'labels']);
  });
});

describe('the layers points may be coloured by', () => {
  it('are the drawable layers that are not labels layers', () => {
    const clusters = {...layer('clusters'), computedContent: ['centroid']};
    const districts = {...layer('districts'), computedContent: ['box']};
    const roster = [clusters, layer('labels', ['clusters']), layer('descriptors'), districts];
    expect(colourLayers(roster).map((l) => l.name)).toEqual(['clusters', 'districts']);
  });
});

describe('the clusters a map shows', () => {
  it('is the canvas’s area in squares of the spacing, rounded down, and at least one', () => {
    expect(clustersInView(100, 1200, 800)).toBe(96);
    expect(clustersInView(100, 1000, 800)).toBe(80);
    expect(clustersInView(200, 1000, 800)).toBe(20);
    expect(clustersInView(100, 150, 150)).toBe(2);
    expect(clustersInView(400, 100, 100)).toBe(1);
  });
});

describe('the level a levelled layer follows', () => {
  const levelled = (name: string, kind: 'tiered' | 'stacked' | 'nested', levels: number[], zoom: (l: number) => [number, number] | null = () => null): Layer =>
    declared(name, {hierarchy: {kind, pruneChildren: false}, levels: levels.map((level) => ({level, title: null, zoom: zoom(level)}))});
  const bands = levelled('bands', 'tiered', [0, 1, 2]);
  const ranged = levelled('ranged', 'tiered', [0, 1, 2], (l) => [l * 4, l * 4 + 3]);
  /** What the tiles in view hold of `name`: each level's count drawn, and whether a tile came back full of it. */
  const inView = (name: string, levels: [number, number, boolean][]): LevelsInView => new Map([[name, new Map(levels.map(([level, served, full]) => [level, {served, full}]))]]);
  const thin = inView('bands', [
    [0, 3, false],
    [1, 12, false],
    [2, 40, false]
  ]);

  it('is the finest level in view that no full tile crowds', () => {
    expect(levelInView(bands, thin, null)).toBe(2);
    expect(levelInView(bands, inView('bands', [[0, 3, false], [1, 12, false], [2, 40, true]]), null)).toBe(1);
    // Every level crowded: the coarsest.
    expect(levelInView(bands, inView('bands', [[0, 3, true], [1, 12, true], [2, 40, true]]), null)).toBe(0);
  });

  it('is the finest level in view with no more artifacts drawn than the cluster count', () => {
    expect(levelInView(bands, thin, 40)).toBe(2);
    expect(levelInView(bands, thin, 39)).toBe(1);
    expect(levelInView(bands, thin, 5)).toBe(0);
    expect(levelInView(bands, thin, 2)).toBe(0);
  });

  it('moves to a finer level only within four fifths of the cluster count, and back only past the whole count', () => {
    // Drawn at level 1, level 2's 40 in view do not move it under a budget of 49, and do under 50.
    expect(levelInView(bands, thin, 49, 1)).toBe(1);
    expect(levelInView(bands, thin, 50, 1)).toBe(2);
    // Drawn at level 2, it stays there while its 40 are within the budget, and steps up past it.
    expect(levelInView(bands, thin, 40, 2)).toBe(2);
    expect(levelInView(bands, thin, 39, 2)).toBe(1);
    // A full tile crowds a level whatever the budget.
    expect(levelInView(bands, inView('bands', [[0, 3, false], [1, 12, false], [2, 4, true]]), 1000, 2)).toBe(1);
  });

  it('is a broad level at world zoom and deepens a level a zoom step at one cluster per 100 pixel square', () => {
    // Four levels of 16, 46, 161 and 574 topics, all in view at world zoom, and a quarter of each
    // with those across the edge a zoom step in, then a sixteenth two steps in.
    const topics = levelled('topics', 'tiered', [0, 1, 2, 3]);
    const at = (served: number[]) => inView('topics', served.map((n, level): [number, number, boolean] => [level, n, false]));
    const world = at([16, 46, 161, 574]);
    const oneIn = at([6, 18, 55, 170]);
    const twoIn = at([3, 6, 18, 60]);
    for (const [width, height] of [[1200, 800], [1000, 800]] as const) {
      const count = clustersInView(100, width, height);
      // The layer starts at its coarsest level, so a finer one must be within four fifths of the count.
      const first = levelInView(topics, world, count, 0)!;
      expect(first).toBe(1);
      const second = levelInView(topics, oneIn, count, first)!;
      expect(second).toBe(2);
      expect(levelInView(topics, twoIn, count, second)).toBe(3);
    }
    // At one per 200 pixel square the world view keeps the 16 broad topics.
    expect(levelInView(topics, world, clustersInView(200, 1200, 800), 0)).toBe(0);
  });

  it('is the coarsest level the camera’s zoom serves where a zoom range is declared, however thin', () => {
    expect(levelInView(ranged, inView('ranged', [[1, 2, false], [2, 1, false]]), null)).toBe(1);
    expect(levelInView(ranged, inView('ranged', [[2, 1, false]]), 1000, 1)).toBe(2);
  });

  it('is the coarsest level while nothing of the layer is in view, and no level on a tree', () => {
    expect(levelInView(bands, new Map(), 10, 2)).toBe(0);
    expect(levelInView(levelled('tree', 'nested', []), thin, 10)).toBeUndefined();
  });

  it('is drawn where no level the layer declares is chosen, and the choice stands otherwise', () => {
    const followed = new Map([['bands', 2]]);
    expect(follows(bands, undefined)).toBe(true);
    expect(follows(bands, 7)).toBe(true);
    expect(follows(bands, 1)).toBe(false);
    expect(follows(ranged, null)).toBe(false);
    expect(levelOfLayer(bands, 7, followed)).toBe(2);
    expect(levelOfLayer(bands, 1, followed)).toBe(1);
    expect(levelOfLayer(bands, null, new Map())).toBe(0);
    expect(levelOfLayer(levelled('tree', 'nested', []), 1, followed)).toBeUndefined();
  });
});
