import type {Store} from '@mosaicajs/client';

export function paper(store: Store): void {
  const panel = document.querySelector<HTMLElement>('#paper')!;
  const heading = panel.querySelector('h2')!;
  const archive = panel.querySelector<HTMLParagraphElement>('.archive')!;
  const link = panel.querySelector('a')!;
  const map = document.querySelector('mosaica-map')!;

  let shown = '';
  map.addEventListener('mosaica-itemopen', async (e) => {
    shown = e.detail.id;
    const fields = e.detail.record.fields;
    heading.textContent = String(fields.title);
    link.href = `https://arxiv.org/abs/${fields.arxiv_id}`;
    archive.textContent = '';
    panel.hidden = false;
    const title = await store.titleOf('archive', String(fields.archive));
    if (shown !== e.detail.id) return;
    archive.textContent = `Archive: ${title ?? fields.archive}`;
  });
  map.addEventListener('mosaica-miss', () => {
    panel.hidden = true;
  });
}
