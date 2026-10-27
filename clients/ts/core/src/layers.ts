import type {Layer} from './types.js';

/**
 * One entry in a layer picker: a layer and the layers linked to it by `depends_on`, which are
 * drawn together. For example, a clustering's labels are a separate layer that depends on the
 * clustering, and one entry covers both.
 *
 * @category Layers and views
 */
export type LayerEntry = {
  /**
   * The entry's main layer: one that depends on no other layer in `meta`. A layer whose
   * dependencies the viewer cannot see counts as a root.
   */
  root: Layer;
  /** `root` and every layer linked to it, as {@link layerClosure} gives them. A layer that depends on two roots is in both entries. */
  closure: string[];
};

/**
 * The layers in `names` together with every layer linked to them by `depends_on`, in either
 * direction and at any distance, each listed once in `layers` order. Names `layers` does not list
 * are kept, at the end.
 *
 * @category Layers and views
 */
export function layerClosure(layers: readonly Layer[], names: readonly string[]): string[] {
  const byName = new Map(layers.map((l) => [l.name, l]));
  const dependents = new Map<string, string[]>();
  for (const l of layers) for (const d of l.depsOn) (dependents.get(d) ?? dependents.set(d, []).get(d)!).push(l.name);
  const seen = new Set<string>();
  const stack = names.filter((n) => byName.has(n));
  while (stack.length > 0) {
    const name = stack.pop()!;
    if (seen.has(name)) continue;
    seen.add(name);
    for (const d of byName.get(name)!.depsOn) if (byName.has(d)) stack.push(d);
    for (const d of dependents.get(name) ?? []) stack.push(d);
  }
  // A name `meta` does not list is kept: the server intersects the request with what the principal
  // reaches, and a host that set a layer before `meta` arrived keeps its choice.
  const unknown = names.filter((n) => !byName.has(n));
  return [...layers.filter((l) => seen.has(l.name)).map((l) => l.name), ...unknown.filter((n, i) => unknown.indexOf(n) === i)];
}

/**
 * The entries for a layer picker: one per layer that depends on no other layer in `layers`, with
 * the layers linked to it, in `layers` order.
 *
 * @category Layers and views
 */
export function layerEntries(layers: readonly Layer[]): LayerEntry[] {
  const reachable = new Set(layers.map((l) => l.name));
  return layers
    .filter((l) => l.depsOn.every((d) => !reachable.has(d)))
    .map((root) => ({root, closure: layerClosure(layers, [root.name])}));
}

/**
 * Whether a layer is a filter layer: one with no computed geometry (`computedContent` is empty)
 * that depends on no other layer. Its artifacts have counts but nothing to draw, so the store does
 * not draw it. Use its artifacts in `member_of` clauses, for example from a hierarchy panel.
 *
 * @category Layers and views
 */
export function isFilterLayer(layer: Layer): boolean {
  // A labels layer also declares no geometry; its text is drawn at the artifact it depends on.
  return layer.computedContent.length === 0 && layer.depsOn.length === 0;
}

/**
 * The layers that can be drawn: every layer except filter layers ({@link isFilterLayer}).
 *
 * @category Layers and views
 */
export function drawableLayers(layers: readonly Layer[]): Layer[] {
  return layers.filter((l) => !isFilterLayer(l));
}

/**
 * The layers points can be coloured by with `setColourBy('cluster:<layer>')`: every layer that is
 * not a filter layer and depends on no other layer. A labels layer depends on the layer it labels,
 * so it is left out.
 *
 * @category Layers and views
 */
export function colourLayers(layers: readonly Layer[]): Layer[] {
  return layers.filter((l) => !isFilterLayer(l) && l.depsOn.length === 0);
}

/**
 * The layers a hierarchy panel can show: every layer with a hierarchy (a `hierarchy.kind` other
 * than `flat`) that depends on no other layer.
 *
 * @category Layers and views
 */
export function browsableLayers(layers: readonly Layer[]): Layer[] {
  return layers.filter((l) => l.depsOn.length === 0 && l.hierarchy.kind !== 'flat');
}

const topLevels = new WeakMap<readonly unknown[], Map<string, number | undefined>>();

/**
 * The top (coarsest) level of a `stacked` or `tiered` layer among those served now: the least
 * level of the layer's artifacts in `served`, which hold only the levels the camera's zoom
 * declares, or the layer's first declared level while none of its artifacts is served.
 * `undefined` on a layer of any other kind, and on one declaring no levels.
 *
 * @internal
 */
export function topLevelServed(layer: Pick<Layer, 'name' | 'hierarchy' | 'levels'>, served: readonly {layer: string; rung: number}[]): number | undefined {
  if ((layer.hierarchy.kind !== 'stacked' && layer.hierarchy.kind !== 'tiered') || layer.levels.length === 0) return undefined;
  let held = topLevels.get(served);
  if (!held) topLevels.set(served, (held = new Map()));
  if (held.has(layer.name)) return held.get(layer.name);
  let top: number | undefined;
  for (const a of served) if (a.layer === layer.name && (top === undefined || a.rung < top)) top = a.rung;
  top ??= layer.levels[0]!.level;
  held.set(layer.name, top);
  return top;
}
