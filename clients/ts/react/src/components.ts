/**
 * `@mosaicajs/react/components` is every element of `@mosaicajs/components` as a React component,
 * such as `MosaicaMap` for `<mosaica-map>`. Each component takes the element's properties as props,
 * including objects such as `store` that React 18 would otherwise set as attributes. Each event
 * the element fires is a handler prop, such as `onPick` for `mosaica-pick`. The element's
 * attributes, properties, events, slots and parts are on its page in the Components reference,
 * which each component's type below links to.
 *
 * A component draws from the store you pass as `store`, such as one from `useMosaicaStore` in
 * `@mosaicajs/react`, or else from an enclosing `MosaicaStore` or `MosaicaExplorer`. Those two and
 * `MosaicaMap` can also make their own store from `viewerUrl` and `token` or `authorise`. You can
 * pass an inline `authorise` function: the component calls the latest one and does not create a
 * new store when it changes.
 * To show the map to another viewer, change `token` or `viewerUrl`, or give the component a new
 * `key`.
 *
 * The element classes are exported as types with an `Element` suffix, such as `MapElement`, for
 * typing a ref. This entry imports `@mosaicajs/components`, and with it Lit and deck.gl.
 *
 * @example
 * ```tsx
 * import {useMosaicaStore} from '@mosaicajs/react';
 * import {MosaicaMap, MosaicaItemCard} from '@mosaicajs/react/components';
 *
 * function Map({token}: {token: string}) {
 *   const store = useMosaicaStore({viewerUrl: 'http://localhost:8080', token});
 *   return (
 *     <>
 *       <MosaicaMap store={store} onPick={(e) => console.log(e.detail.id)} />
 *       <MosaicaItemCard store={store} />
 *     </>
 *   );
 * }
 * ```
 *
 * @module @mosaicajs/react/components
 */
import {createComponent, type EventName} from '@lit/react';
import * as React from 'react';
import {
  MosaicaArtifactCard as ArtifactCardElement,
  MosaicaClusterFilter as ClusterFilterElement,
  MosaicaColourEditor as ColourEditorElement,
  MosaicaCount as CountElement,
  MosaicaExplorer as ExplorerElement,
  MosaicaFieldCard as FieldCardElement,
  MosaicaFilter as FilterElement,
  MosaicaFilterPanel as FilterPanelElement,
  MosaicaHierarchy as HierarchyElement,
  MosaicaItemCard as ItemCardElement,
  MosaicaKeyPicker as KeyPickerElement,
  MosaicaLayerPicker as LayerPickerElement,
  MosaicaMap as MapElement,
  MosaicaSelection as SelectionElement,
  MosaicaStatus as StatusElement,
  MosaicaStore as StoreElement,
  MosaicaViewPicker as ViewPickerElement,
  type MosaicaEventMap
} from '@mosaicajs/components';
import type {TokenSupplier} from '@mosaicajs/client';


/**
 * The events the elements fire, by name, each a `CustomEvent` with its `detail`. The same type as
 * `MosaicaEventMap` in `@mosaicajs/components`.
 */
export type MosaicaEvents = MosaicaEventMap;

const ev = <K extends keyof MosaicaEvents>(name: K) => name as EventName<MosaicaEvents[K]>;

const wrap = <E extends HTMLElement, Ev extends Record<string, EventName>>(tagName: string, elementClass: new () => E, events: Ev) =>
  createComponent({react: React, tagName, elementClass, events, displayName: elementClass.name});

/**
 * Hands the element one function per mount in place of the `authorise` prop, and calls the latest
 * prop through a ref, as `useMosaicaStore` does. An element builds a new store when its `authorise`
 * function changes, so an inline arrow passed straight through would build one on every render.
 * The store is built again when `authorise` is given or removed; to show another viewer, change
 * `token`, `viewerUrl` or the component's `key`.
 */
const withStableAuthorise = <C extends React.ForwardRefExoticComponent<any>>(Inner: C): C => {
  const Outer = React.forwardRef<unknown, {authorise?: TokenSupplier | null}>((props, ref) => {
    const latest = React.useRef(props.authorise);
    React.useLayoutEffect(() => {
      latest.current = props.authorise;
    });
    const given = props.authorise != null;
    const authorise = React.useMemo(() => (given ? () => latest.current!() : null), [given]);
    return React.createElement(Inner, {...props, authorise, ref});
  });
  Outer.displayName = Inner.displayName;
  return Outer as unknown as C;
};

/** The handler props for events a map emits, which the explorer also carries. */
const mapEvents = {
  onPick: ev('mosaica-pick'),
  onItemOpen: ev('mosaica-itemopen'),
  onMiss: ev('mosaica-miss'),
  onHover: ev('mosaica-hover'),
  onViewChange: ev('mosaica-viewchange'),
  onSelectChange: ev('mosaica-selectchange'),
  onArtifactOpen: ev('mosaica-artifactopen'),
  onLayerChange: ev('mosaica-layerchange')
};

/** `<mosaica-store>` as a React component. It has no event props. */
export const MosaicaStore = withStableAuthorise(wrap('mosaica-store', StoreElement, {}));
/**
 * `<mosaica-explorer>` as a React component. The events of the elements inside it bubble out, so
 * it has a handler prop for each of them: `onPick` for `mosaica-pick`, and so on for every event
 * in {@link MosaicaEvents} except `mosaica-chipopen` and `mosaica-basemapchange`. The `open` prop
 * is the function the item card's Open button calls, and `openUrl` the address it goes to.
 * `onOpen` is called on every press.
 */
export const MosaicaExplorer = withStableAuthorise(wrap('mosaica-explorer', ExplorerElement, {
  ...mapEvents,
  onArtifactFit: ev('mosaica-artifactfit'),
  onColourChange: ev('mosaica-colourchange'),
  onLevelChange: ev('mosaica-levelchange'),
  onValueColour: ev('mosaica-valuecolour'),
  onClusterColour: ev('mosaica-clustercolour'),
  onPaletteChange: ev('mosaica-palettechange'),
  onClusterPaletteChange: ev('mosaica-clusterpalettechange'),
  onDisplayChange: ev('mosaica-displaychange'),
  onSizeChange: ev('mosaica-sizechange'),
  onBudgetChange: ev('mosaica-budgetchange'),
  onClusterBudgetChange: ev('mosaica-clusterbudgetchange'),
  onFold: ev('mosaica-fold'),
  onFilterChange: ev('mosaica-filterchange'),
  onStateChange: ev('mosaica-statechange'),
  onExpired: ev('mosaica-expired'),
  onOpen: ev('mosaica-open'),
  onClose: ev('mosaica-close'),
  onClauseChange: ev('mosaica-clausechange'),
  onViewSwitch: ev('mosaica-viewswitch'),
  onViewFollow: ev('mosaica-viewfollow')
}));
/**
 * `<mosaica-map>` as a React component. Event props: `onPick` (`mosaica-pick`), `onItemOpen`
 * (`mosaica-itemopen`), `onMiss` (`mosaica-miss`), `onHover` (`mosaica-hover`), `onViewChange`
 * (`mosaica-viewchange`), `onSelectChange` (`mosaica-selectchange`), `onArtifactOpen`
 * (`mosaica-artifactopen`) and `onLayerChange` (`mosaica-layerchange`).
 */
export const MosaicaMap = withStableAuthorise(wrap('mosaica-map', MapElement, mapEvents));
/**
 * `<mosaica-status>` as a React component. Event props: `onStateChange` (`mosaica-statechange`)
 * and `onExpired` (`mosaica-expired`).
 */
export const MosaicaStatus = wrap('mosaica-status', StatusElement, {onStateChange: ev('mosaica-statechange'), onExpired: ev('mosaica-expired')});
/** `<mosaica-count>` as a React component. It has no event props. */
export const MosaicaCount = wrap('mosaica-count', CountElement, {});
/**
 * `<mosaica-item-card>` as a React component. Event props: `onOpen` (`mosaica-open`), `onClose`
 * (`mosaica-close`) and `onViewFollow` (`mosaica-viewfollow`). The `open` prop is the function the
 * Open button calls, and `openUrl` the address it goes to. `onOpen` is called on every press.
 */
export const MosaicaItemCard = wrap('mosaica-item-card', ItemCardElement, {onOpen: ev('mosaica-open'), onClose: ev('mosaica-close'), onViewFollow: ev('mosaica-viewfollow')});
/**
 * `<mosaica-filter-panel>` as a React component. Event props: `onFilterChange`
 * (`mosaica-filterchange`), `onChipOpen` (`mosaica-chipopen`), and from its cards
 * `onClauseChange` (`mosaica-clausechange`), `onColourChange` (`mosaica-colourchange`),
 * `onLevelChange` (`mosaica-levelchange`), `onValueColour` (`mosaica-valuecolour`),
 * `onClusterColour` (`mosaica-clustercolour`) and `onFold` (`mosaica-fold`).
 */
export const MosaicaFilterPanel = wrap('mosaica-filter-panel', FilterPanelElement, {
  onFilterChange: ev('mosaica-filterchange'),
  onChipOpen: ev('mosaica-chipopen'),
  onClauseChange: ev('mosaica-clausechange'),
  onColourChange: ev('mosaica-colourchange'),
  onLevelChange: ev('mosaica-levelchange'),
  onValueColour: ev('mosaica-valuecolour'),
  onClusterColour: ev('mosaica-clustercolour'),
  onFold: ev('mosaica-fold')
});
/** `<mosaica-filter>` as a React component. Event prop: `onFilterChange` (`mosaica-filterchange`). */
export const MosaicaFilter = wrap('mosaica-filter', FilterElement, {onFilterChange: ev('mosaica-filterchange')});
/** `<mosaica-cluster-filter>` as a React component. Event prop: `onClauseChange` (`mosaica-clausechange`). */
export const MosaicaClusterFilter = wrap('mosaica-cluster-filter', ClusterFilterElement, {onClauseChange: ev('mosaica-clausechange')});
/** `<mosaica-selection>` as a React component. Event prop: `onSelectChange` (`mosaica-selectchange`). */
export const MosaicaSelection = wrap('mosaica-selection', SelectionElement, {onSelectChange: ev('mosaica-selectchange')});
/** `<mosaica-layer-picker>` as a React component. Event prop: `onLayerChange` (`mosaica-layerchange`). */
export const MosaicaLayerPicker = wrap('mosaica-layer-picker', LayerPickerElement, {onLayerChange: ev('mosaica-layerchange')});
/** `<mosaica-view-picker>` as a React component. Event prop: `onViewSwitch` (`mosaica-viewswitch`). */
export const MosaicaViewPicker = wrap('mosaica-view-picker', ViewPickerElement, {onViewSwitch: ev('mosaica-viewswitch')});
/** `<mosaica-key-picker>` as a React component. Event prop: `onViewSwitch` (`mosaica-viewswitch`). */
export const MosaicaKeyPicker = wrap('mosaica-key-picker', KeyPickerElement, {onViewSwitch: ev('mosaica-viewswitch')});
/**
 * `<mosaica-artifact-card>` as a React component. Event props: `onArtifactFit`
 * (`mosaica-artifactfit`), `onClauseChange` (`mosaica-clausechange`) and `onClose`
 * (`mosaica-close`).
 */
export const MosaicaArtifactCard = wrap('mosaica-artifact-card', ArtifactCardElement, {
  onArtifactFit: ev('mosaica-artifactfit'),
  onClauseChange: ev('mosaica-clausechange'),
  onClose: ev('mosaica-close')
});
/**
 * `<mosaica-field-card>` as a React component. Event props: `onFilterChange`
 * (`mosaica-filterchange`), `onClauseChange` (`mosaica-clausechange`), `onColourChange`
 * (`mosaica-colourchange`), `onLevelChange` (`mosaica-levelchange`), `onValueColour`
 * (`mosaica-valuecolour`), `onClusterColour` (`mosaica-clustercolour`) and `onFold` (`mosaica-fold`).
 */
export const MosaicaFieldCard = wrap('mosaica-field-card', FieldCardElement, {
  onFold: ev('mosaica-fold'),
  onFilterChange: ev('mosaica-filterchange'),
  onClauseChange: ev('mosaica-clausechange'),
  onColourChange: ev('mosaica-colourchange'),
  onLevelChange: ev('mosaica-levelchange'),
  onValueColour: ev('mosaica-valuecolour'),
  onClusterColour: ev('mosaica-clustercolour')
});
/**
 * `<mosaica-colour-editor>` as a React component. Event props: `onValueColour`
 * (`mosaica-valuecolour`) and `onClusterColour` (`mosaica-clustercolour`).
 */
export const MosaicaColourEditor = wrap('mosaica-colour-editor', ColourEditorElement, {
  onValueColour: ev('mosaica-valuecolour'),
  onClusterColour: ev('mosaica-clustercolour')
});
/**
 * `<mosaica-hierarchy>` as a React component. Event props: `onClauseChange` (`mosaica-clausechange`)
 * and `onArtifactFit` (`mosaica-artifactfit`).
 */
export const MosaicaHierarchy = wrap('mosaica-hierarchy', HierarchyElement, {onClauseChange: ev('mosaica-clausechange'), onArtifactFit: ev('mosaica-artifactfit')});

export type {
  ArtifactCardElement,
  ClusterFilterElement,
  ColourEditorElement,
  CountElement,
  ExplorerElement,
  FieldCardElement,
  FilterElement,
  FilterPanelElement,
  HierarchyElement,
  ItemCardElement,
  KeyPickerElement,
  LayerPickerElement,
  MapElement,
  SelectionElement,
  StatusElement,
  StoreElement,
  ViewPickerElement
};
