import type {Store} from '@mosaicajs/client';

export function topics(store: Store): void {
  const map = document.querySelector('mosaica-map')!;
  const toggle = document.querySelector<HTMLInputElement>('#topics')!;
  const chosen = document.querySelector<HTMLParagraphElement>('#topic')!;

  map.colourBy = 'cluster:clusters/toponymy';

  toggle.addEventListener('change', () => {
    map.layers = toggle.checked ? ['topics/toponymy'] : [];
  });

  map.addEventListener('mosaica-artifactopen', (e) => {
    const artifact = BigInt(e.detail.id);
    const name = store.get('artifacts').attached.get(artifact) ?? 'Unnamed topic';
    store.setMembers([{layer: e.detail.detail.layer, artifact, outside: false, verb: 'filter', label: name}]);
    chosen.querySelector('strong')!.textContent = name;
    chosen.hidden = false;
  });

  chosen.querySelector('button')!.addEventListener('click', () => {
    store.setMembers([]);
    chosen.hidden = true;
  });
}
