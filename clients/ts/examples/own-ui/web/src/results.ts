import {formatMasked, type Store} from '@mosaicajs/client';

export function results(store: Store): void {
  const count = document.querySelector<HTMLParagraphElement>('#count')!;
  const list = document.querySelector<HTMLOListElement>('#results')!;
  const map = document.querySelector('mosaica-map')!;

  // How many papers in the part of the map on screen match the filters.
  store.subscribe('view', (view) => {
    if (view.inView?.status !== 'shown') return;
    const matched = formatMasked(view.inView.matched);
    const visible = formatMasked(view.inView.visible);
    count.textContent = `${matched} of the ${visible} papers in view match`;
  });

  let cameraSet = false;
  let reading: AbortController | null = null;
  let timer = 0;

  // The titles of the first 20 of those papers, in the order they lie on the map.
  async function listFirst(): Promise<void> {
    if (!cameraSet) return;
    reading?.abort();
    const controller = (reading = new AbortController());
    try {
      const read = await store.items({fields: ['title'], inView: true, order: 'map', pageRows: 20, pages: 1}, controller.signal);
      const page = await read.next();
      await read.return();
      const titles: string[] = page.done ? [] : page.value.getChild('title')!.toArray();
      list.replaceChildren(...titles.map((title) => Object.assign(document.createElement('li'), {textContent: title})));
    } catch (error) {
      if (!controller.signal.aborted) throw error;
    }
  }

  // Ask once the camera or the filters have rested for a quarter of a second.
  function soon(): void {
    window.clearTimeout(timer);
    timer = window.setTimeout(listFirst, 250);
  }
  map.addEventListener('mosaica-viewchange', () => {
    cameraSet = true;
    soon();
  });
  store.subscribe('filters', soon);
}
