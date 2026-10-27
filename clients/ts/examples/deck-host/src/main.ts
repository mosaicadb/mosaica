import {Deck, OrthographicView} from '@deck.gl/core';
import {WORLD_SIZE, createStore, type Store} from '@mosaicajs/client';
import {MosaicaLayer, cameraFitting, frameMatches, viewInputOf} from '@mosaicajs/deck';
import {fitWorld, worldEdge, type ViewState} from './view.js';

/**
 * `MosaicaLayer` in a `Deck` the host builds. The host owns the view and the camera, tells the
 * store where the camera is through `viewInputOf`, and draws a layer of its own under the marks.
 * Each sign-in opens the camera on the items that viewer may see, through `frameMatches`.
 * The layer makes its GPU buffers on the deck's device and releases them itself.
 */
const parent = document.getElementById('map') as HTMLDivElement;
const size = () => ({width: parent.clientWidth || 1, height: parent.clientHeight || 1});

let store: Store | null = null;
let viewState: ViewState = fitWorld(size().width, size().height);
/**
 * Whether the camera waits for the matched items to be framed. Nothing is sent to the store
 * meanwhile; a move by the user ends the wait and keeps the camera where it was moved.
 */
let waiting = false;

const deck = new Deck<OrthographicView>({
  parent,
  views: new OrthographicView({id: 'ortho', flipY: true}),
  viewState,
  controller: true,
  layers: [],
  onViewStateChange: ({viewState: next}) => {
    const v = next as {target: number[]; zoom: number};
    viewState = {...viewState, target: [v.target[0]!, v.target[1]!, 0], zoom: v.zoom};
    waiting = false;
    deck.setProps({viewState});
    tell();
    return next;
  }
});

/**
 * Tell the store where the camera is looking. Nothing is sent before `meta` gives it a frame, or
 * while the camera waits to be framed.
 */
function tell(): void {
  if (!store || waiting) return;
  const {width, height} = size();
  const input = viewInputOf(store, viewState, width, height);
  if (input) store.setView(input);
}

/** The layer list: the host's own layer, then Mosaica's over the current store. */
function draw(): void {
  deck.setProps({layers: [worldEdge(), store ? new MosaicaLayer({id: 'mosaica', store}) : null]});
}

/** The newest sign-in asked for; an older one's token or frame that lands later is dropped. */
let opening = 0;
/** Cancels the framing of the store last opened. */
let framing = new AbortController();

async function open(user: string): Promise<void> {
  const ticket = ++opening;
  framing.abort();
  framing = new AbortController();
  const signal = framing.signal;
  store?.dispose();
  store = null;
  draw();
  const {token} = (await (await fetch(`/token?user=${encodeURIComponent(user)}`, {method: 'POST'})).json()) as {token: string};
  if (ticket !== opening) return;
  const s = createStore({viewerUrl: location.origin, token});
  store = s;
  waiting = true;
  draw();
  // The first view goes out once the matched items are framed, which waits for `meta` and the
  // store's frame with it. Where nothing matches, the whole world square is shown.
  const box = await frameMatches(s, {signal});
  if (ticket !== opening) return;
  if (waiting) {
    waiting = false;
    const {width, height} = size();
    viewState = {...viewState, ...cameraFitting(box ?? [0, 0, WORLD_SIZE, WORLD_SIZE], width, height)};
    deck.setProps({viewState});
  }
  tell();
}

new ResizeObserver(() => tell()).observe(parent);

const principal = document.getElementById('principal') as HTMLSelectElement;
const users = (await (await fetch('/users')).json()) as {name: string; label: string}[];
for (const u of users) principal.add(new Option(u.label, u.name));
principal.value = users.at(-1)?.name ?? '';
principal.addEventListener('change', () => void open(principal.value));
await open(principal.value);
