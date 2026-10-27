import {afterEach, beforeAll, beforeEach, describe, expect, it, vi} from 'vitest';
import {LayerManager, type Layer} from '@deck.gl/core';
import {WORLD_SIZE, type ArtifactsProjection, type MarksProjection} from '@mosaicajs/client';
import {SessionArtifactTable, mortonOfTile, servedLineage} from '@mosaicajs/client/internal';
import {band} from '../../core/test/support.js';
import {contourThresholds, densityPosition, densitySteps, densityStops, type DensityCell, type DensityCounts} from '../src/density.js';
import {MosaicaLayer, loadAggregationLayers, type MosaicaLayerInternalProps} from '../src/layer.js';
import {fakeDevice} from './fake-device.js';

/**
 * Every density mode draws the counts by cell it is given. The marks are a sample capped per tile,
 * so the tests serve a sample that disagrees with the counts: a cell with three marks and a large
 * count beside a cell with many marks and a small one. What each mode was given to draw is read off
 * the sublayer the Mosaica layer rendered.
 */

const DEPTH = 3;
/** The counts' depth, finer than the marks' bands. */
const CELLS = DEPTH + 2;
const SPAN = WORLD_SIZE / 2 ** CELLS;

const cell = (x: number, y: number, count: number): DensityCell => ({x, y, position: [(x + 0.5) * SPAN, (y + 0.5) * SPAN], count});

/** Few marks where the count is large, many where it is small. */
const counts: DensityCounts = {depth: CELLS, cells: [cell(4, 4, 80_000), cell(8, 4, 300)]};
const marks: MarksProjection = {
  bands: [band(DEPTH, mortonOfTile(1, 1, DEPTH), 3), band(DEPTH, mortonOfTile(2, 1, DEPTH), 400)],
  standIn: [],
  count: {shown: 403, total: 80_300, exact: true}
};

function artifacts(): ArtifactsProjection {
  return {
    layer: null,
    layers: [],
    served: [],
    colourServed: [],
    attached: new Map(),
    lineage: servedLineage([]),
    status: 'idle',
    refusal: null,
    version: 0,
    held: 0,
    table: new SessionArtifactTable(),
    servedOrdinals: new Set(),
    shapes: new Map(),
    colours: new Map(),
    palette: 'tableau10', overrides: new Map(),
    coverage: {current: 0, stale: 0}
  };
}

type Cell = DensityCell;

function host() {
  const manager = new LayerManager(fakeDevice(), {});
  const errors: unknown[] = [];
  manager.setProps({onError: (error: unknown, layer: Layer) => (layer.id === 'mosaica' ? errors.push(error) : undefined)});
  const sublayer = (id: string) => {
    const layer = manager.getLayers().find((l) => l.id === 'mosaica') as MosaicaLayer | undefined;
    return (layer?.getSubLayers() as Layer[] | undefined)?.find((l) => l.id === `mosaica-${id}`);
  };
  const props = <T>(id: string) => sublayer(id)?.props as unknown as T;
  return {
    errors,
    sublayer,
    props,
    /** Draw, and draw again once the cells have settled, as a host's next frame would. */
    draw: (props: Partial<MosaicaLayerInternalProps>) => {
      const layer = () => new MosaicaLayer({id: 'mosaica', depth: DEPTH, status: 'shown', artifacts: artifacts(), marks, densityCounts: counts, ...props} as MosaicaLayerInternalProps);
      manager.setLayers([layer()]);
      vi.advanceTimersByTime(1000);
      manager.setLayers([layer()]);
    },
    /** Draw once, with no time to settle. */
    drawNow: (props: Partial<MosaicaLayerInternalProps>) =>
      manager.setLayers([new MosaicaLayer({id: 'mosaica', depth: DEPTH, status: 'shown', artifacts: artifacts(), marks, densityCounts: counts, ...props} as MosaicaLayerInternalProps)]),
    /**
     * The grid's colour at a world point, `[r, g, b, a]`, read from the image it draws; `null`
     * where no grid is drawn.
     */
    gridAt: (world: [number, number]): number[] | null => {
      const props = sublayer('density-grid')?.props as unknown as {visible: boolean; image: {data: Uint8ClampedArray; width: number; height: number}; bounds: [number, number, number, number]} | undefined;
      if (!props?.visible) return null;
      const [left, bottom, right, top] = props.bounds;
      const {data, width, height} = props.image;
      const px = Math.floor(((world[0] - left) / (right - left)) * width);
      const py = Math.floor(((world[1] - top) / (bottom - top)) * height);
      if (px < 0 || py < 0 || px >= width || py >= height) return [0, 0, 0, 0];
      const i = (py * width + px) * 4;
      return [...data.slice(i, i + 4)];
    },
    /**
     * What an aggregation sublayer was given: each cell's position and its count, as the contours'
     * weight accessor reads it or as the hexagons' cells carry it.
     */
    aggregated: (id: string): {position: [number, number]; weight: number}[] | null => {
      const props = sublayer(id)?.props as {data: Cell[]; getPosition: (d: Cell) => [number, number]; getWeight?: (d: Cell) => number} | undefined;
      if (!props) return null;
      const weight = props.getWeight ?? ((d: Cell) => d.count);
      return props.data.map((d) => ({position: props.getPosition(d), weight: weight(d)}));
    },
    /** The hexagons drawn: each one's centre and colour, and their radius, centre to corner. */
    hexagons: (): {radius: number; hexes: {position: [number, number]; colour: number[]}[]} => {
      const props = sublayer('density-hex')!.props as unknown as {
        data: {length: number; attributes: {getPosition: {value: ArrayLike<number>}}};
        radius: number;
        getFillColor: (d: unknown, info: {index: number}) => number[];
      };
      const at = props.data.attributes.getPosition.value;
      return {
        radius: props.radius,
        hexes: Array.from({length: props.data.length}, (_, index) => ({position: [at[2 * index]!, at[2 * index + 1]!] as [number, number], colour: [...props.getFillColor(null, {index})]}))
      };
    }
  };
}

const centre = (x: number, y: number): [number, number] => [(x + 0.5) * SPAN, (y + 0.5) * SPAN];

describe('density modes', () => {
  beforeAll(async () => {
    await loadAggregationLayers();
  });
  beforeEach(() => {
    vi.useFakeTimers();
    vi.stubGlobal('ImageData', class {
      constructor(readonly data: Uint8ClampedArray, readonly width: number, readonly height: number) {}
    });
  });
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it('contours aggregate one point per cell, weighted by its count, whatever the sample holds', () => {
    const h = host();
    h.draw({density: 'contours'});
    expect(h.errors).toEqual([]);
    expect(h.aggregated('density-contours')).toEqual([
      {position: centre(4, 4), weight: 80_000},
      {position: centre(8, 4), weight: 300}
    ]);
  });

  it('draws the grid as one image, a square per cell coloured by the cell’s count, whatever the sample holds', () => {
    const h = host();
    h.draw({density: 'grid', densityColours: 'greys', scheme: 'light'});
    const dense = h.gridAt(centre(4, 4))!;
    const sparse = h.gridAt(centre(8, 4))!;
    expect(dense[3]).toBe(255);
    expect(sparse[3]).toBe(255);
    // A cell with no count is clear.
    expect(h.gridAt(centre(6, 4))![3]).toBe(0);
    // On a light ground the dense end of Greys is the darker: the three-mark cell of 80,000.
    const lum = (c: number[]) => c[0]! + c[1]! + c[2]!;
    expect(lum(dense)).toBeLessThan(lum(sparse));
  });

  it('feeds the contours at most their budget of cells, merged to a coarser depth, every item kept', () => {
    const h = host();
    const many: DensityCell[] = [];
    for (let x = 0; x < 160; x++) for (let y = 0; y < 100; y++) many.push(cell(x, y, 1));
    const fine = {depth: 10, cells: many.map((c) => ({...c, position: [(c.x + 0.5) * (WORLD_SIZE / 1024), (c.y + 0.5) * (WORLD_SIZE / 1024)] as [number, number]}))};
    h.draw({density: 'contours', densityCounts: fine});
    const fed = h.aggregated('density-contours')!;
    expect(fed.length).toBeLessThan(many.length);
    expect(fed.length).toBeLessThanOrEqual(2_000);
    expect(fed.reduce((n, c) => n + c.weight, 0)).toBe(many.length);
  });

  it('draws hexagons of one cell’s area at the counts’ depth, however many cells there are', () => {
    const h = host();
    h.draw({density: 'hex'});
    expect(h.errors).toEqual([]);
    const area = (r: number) => ((3 * Math.sqrt(3)) / 2) * r * r;
    expect(area(h.hexagons().radius)).toBeCloseTo(SPAN * SPAN);
    // 300,000 cells at depth 12 are not merged: the hexagons stay one depth-12 cell in area.
    const span = WORLD_SIZE / 4096;
    const many: DensityCell[] = [];
    for (let x = 0; x < 600; x++) for (let y = 0; y < 500; y++) many.push({x, y, position: [(x + 0.5) * span, (y + 0.5) * span], count: 1});
    h.draw({density: 'hex', densityCounts: {depth: 12, cells: many}});
    expect(area(h.hexagons().radius)).toBeCloseTo(span * span);
  });

  it('draws the counts it is given and nothing without them', () => {
    const h = host();
    h.draw({density: 'grid'});
    expect(h.gridAt(centre(4, 4))![3]).toBe(255);
    h.draw({density: 'grid', densityCounts: {depth: CELLS, cells: [cell(1, 1, 10)]}});
    expect(h.gridAt(centre(1, 1))![3]).toBe(255);
    expect(h.gridAt(centre(4, 4))![3]).toBe(0);
    h.drawNow({density: 'grid', densityCounts: null});
    expect(h.gridAt(centre(1, 1))).toBeNull();
  });

  it.each(['linear', 'log'] as const)('draws contours at evenly spaced positions of the %s scale over the cells', (densityScale) => {
    const h = host();
    h.draw({density: 'contours', densityScale});
    const contours = h.props<{contours: {threshold: number}[]}>('density-contours').contours;
    expect(contours.map((c) => c.threshold)).toEqual(contourThresholds(counts.cells, densityScale));
    expect(contours.map((c) => densityPosition(c.threshold, 80_000, densityScale))).toEqual([0.2, 0.4, 0.6, 0.8].map((p) => expect.closeTo(p, 9)));
  });

  it('draws contours on the log scale where none is set', () => {
    const h = host();
    h.draw({density: 'contours'});
    expect(h.props<{contours: {threshold: number}[]}>('density-contours').contours.map((c) => c.threshold)).toEqual(contourThresholds(counts.cells, 'log'));
  });

  it.each(['linear', 'log'] as const)('colours each hexagon by the %s position of its count, the densest at the top, whatever the sample holds', (densityScale) => {
    const h = host();
    h.draw({density: 'hex', densityScale, densityColours: 'viridis', scheme: 'dark'});
    const steps = densitySteps(densityStops('viridis', 'dark'), 8);
    const colourOf = (count: number) => [...steps[Math.min(7, Math.floor(densityPosition(count, 80_000, densityScale) * 8))]!];
    const near = (x: number, y: number) => {
      const [cx, cy] = centre(x, y);
      return h.hexagons().hexes.filter((hex) => Math.hypot(hex.position[0] - cx, hex.position[1] - cy) < SPAN);
    };
    expect(near(4, 4).length).toBeGreaterThan(0);
    for (const hex of near(4, 4)) expect(hex.colour).toEqual(colourOf(80_000));
    for (const hex of near(8, 4)) expect(hex.colour).toEqual(colourOf(300));
    expect(colourOf(80_000)).toEqual([...steps[7]!]);
  });

  it('colours the hexagons again when only the scale changes, and keeps their data across repaints', () => {
    const h = host();
    const data = () => h.props<{data: unknown}>('density-hex').data;
    const sparse = () => {
      const [cx, cy] = centre(8, 4);
      return h.hexagons().hexes.find((hex) => Math.hypot(hex.position[0] - cx, hex.position[1] - cy) < SPAN)!.colour;
    };
    h.draw({density: 'hex', densityScale: 'linear', densityColours: 'viridis'});
    const linear = sparse();
    const first = data();
    h.draw({density: 'hex', densityScale: 'linear', densityColours: 'viridis'});
    expect(data()).toBe(first);
    h.draw({density: 'hex', densityScale: 'log', densityColours: 'viridis'});
    expect(sparse()).not.toEqual(linear);
    expect(h.props<{updateTriggers: {getFillColor: string}}>('density-hex').updateTriggers.getFillColor).toContain('log');
  });

  it('washes a cell more strongly when only the scale changes from linear to log', () => {
    const h = host();
    const alphaAt = (world: [number, number]) => {
      const props = h.sublayer('wash')!.props as unknown as {image: {data: Uint8ClampedArray; width: number; height: number}; bounds: [number, number, number, number]};
      const [left, bottom, right, top] = props.bounds;
      const {data, width, height} = props.image;
      const px = Math.floor(((world[0] - left) / (right - left)) * width);
      const py = Math.floor(((world[1] - top) / (bottom - top)) * height);
      return data[(py * width + px) * 4 + 3]!;
    };
    h.draw({density: 'smooth', densityScale: 'linear'});
    const linear = alphaAt(centre(8, 4));
    const denseLinear = alphaAt(centre(4, 4));
    h.draw({density: 'smooth', densityScale: 'log'});
    expect(alphaAt(centre(8, 4))).toBeGreaterThan(linear);
    expect(alphaAt(centre(4, 4))).toBe(denseLinear);
  });

  it('colours the grid by each cell’s position on the scale asked for', () => {
    const h = host();
    const lum = (c: number[]) => c[0]! + c[1]! + c[2]!;
    h.draw({density: 'grid', densityColours: 'greys', scheme: 'light', densityScale: 'linear'});
    const linear = lum(h.gridAt(centre(8, 4))!);
    h.draw({density: 'grid', densityColours: 'greys', scheme: 'light', densityScale: 'log'});
    const log = lum(h.gridAt(centre(8, 4))!);
    // 300 of 80,000 is near the sparse, light end on a linear scale and past the middle on a log one.
    expect(log).toBeLessThan(linear);
  });

  it('draws no aggregation layer under none or smooth, and the wash only under smooth', () => {
    const h = host();
    for (const density of ['none', 'smooth'] as const) {
      h.draw({density});
      expect(h.sublayer('density-hex') ?? h.sublayer('density-contours')).toBeUndefined();
      expect(h.gridAt(centre(4, 4))).toBeNull();
      expect((h.sublayer('wash')!.props as {visible: boolean}).visible).toBe(density === 'smooth');
    }
  });

  it('draws density and no marks with the points off', () => {
    const h = host();
    h.draw({density: 'grid', points: false});
    expect(h.errors).toEqual([]);
    expect(h.gridAt(centre(4, 4))![3]).toBe(255);
    const drawnMarks = (h.sublayer('marks-p0')?.props as {visible: boolean} | undefined)?.visible ?? false;
    expect(drawnMarks).toBe(false);
    h.draw({density: 'grid', points: true});
    expect((h.sublayer('marks-p0')!.props as {visible: boolean}).visible).toBe(true);
  });

  it('sets the names on opaque plates with no halo over hexagons, a grid or a ramped wash, and on soft backings under a halo elsewhere', () => {
    const h = host();
    const drawn = () => ({
      plates: h.sublayer('labels-plates') !== undefined,
      backing: h.sublayer('label-backing') !== undefined,
      halo: (h.sublayer('labels')!.props as unknown as {outlineWidth: number}).outlineWidth > 0
    });
    for (const [density, plated] of [['hex', true], ['grid', true], ['smooth', false], ['contours', false], ['none', false]] as const) {
      h.draw({density});
      expect(drawn()).toEqual({plates: plated, backing: !plated, halo: !plated});
    }
    h.draw({density: 'smooth', densityColours: 'viridis'});
    expect(drawn()).toEqual({plates: true, backing: false, halo: false});
  });

  it('draws at the strength asked for', () => {
    const h = host();
    h.draw({density: 'hex', densityStrength: 0.4});
    expect(h.sublayer('density-hex')!.props.opacity).toBe(0.4);
  });
});

describe('densityStops', () => {
  const lum = ([r, g, b]: readonly number[]) => 0.2126 * r! + 0.7152 * g! + 0.0722 * b!;

  it('puts the dense end away from the ground: dark on a light map, light on a dark one', () => {
    for (const colours of ['warm-grey', 'viridis', 'cividis', 'magma', 'greys'] as const) {
      const light = densityStops(colours, 'light');
      const dark = densityStops(colours, 'dark');
      expect(lum(light.at(-1)!)).toBeLessThan(lum(light[0]!));
      expect(lum(dark.at(-1)!)).toBeGreaterThan(lum(dark[0]!));
    }
  });
});
