import type {Store} from '@mosaicajs/client';

export function topics(store: Store): void {
  const map = document.querySelector('mosaica-map')!;
  const toggle = document.querySelector<HTMLInputElement>('#topics')!;
  const chosen = document.querySelector<HTMLSpanElement>('#topic [role="status"]')!;
  const clear = document.querySelector<HTMLButtonElement>('#topic button')!;

  map.colourBy = 'cluster:clusters/toponymy';

  toggle.addEventListener('change', () => {
    map.layers = toggle.checked ? ['topics/toponymy'] : [];
  });

  map.addEventListener('mosaica-artifactopen', (e) => {
    const artifact = BigInt(e.detail.id);
    const name = store.get('artifacts').attached.get(artifact) ?? 'Unnamed topic';
    store.setMembers([{layer: e.detail.detail.layer, artifact, outside: false, verb: 'filter', label: name}]);
    chosen.textContent = `Topic: ${name}`;
    clear.hidden = false;
  });

  clear.addEventListener('click', () => {
    store.setMembers([]);
    chosen.textContent = '';
    clear.hidden = true;
  });
}
