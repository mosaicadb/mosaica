import type {Store} from '@mosaicajs/client';

const DAY_MS = 86_400_000;

/** Microseconds since 1970 at the start of a date input's day, or `null` where it is empty. */
function startOf(date: string): number | null {
  return date ? Date.parse(date) * 1000 : null;
}

/** Microseconds since 1970 at the last instant of a date input's day, or `null` where it is empty. */
function endOf(date: string): number | null {
  return date ? (Date.parse(date) + DAY_MS) * 1000 - 1 : null;
}

export function filters(store: Store): void {
  const archive = document.querySelector<HTMLSelectElement>('#archive')!;
  const title = document.querySelector<HTMLInputElement>('#title')!;
  const from = document.querySelector<HTMLInputElement>('#from')!;
  const to = document.querySelector<HTMLInputElement>('#to')!;

  // Ask how many papers each archive holds, and list the archives in that order.
  store.setAggregate('archives', {
    subject: 'visible',
    groupings: [{count: true, by: {field: 'archive', top: 50}}]
  });
  const unsubscribe = store.subscribe('aggregates', (aggregates) => {
    const rows = aggregates.get('archives')?.result?.tables[0]?.rows;
    if (!rows) return;
    for (const row of rows) {
      if (row.group !== 'listed') continue;
      const count = Number(row.count).toLocaleString('en-GB');
      archive.add(new Option(`${row.title ?? row.key} (${count})`, row.key));
    }
    unsubscribe();
  });

  function apply(): void {
    store.setFilters({
      filter: {
        archive: {family: 'category', keys: archive.value ? [archive.value] : []},
        title: {family: 'text', query: title.value, phrase: true},
        submitted_at: {family: 'numeric', gte: startOf(from.value), lte: endOf(to.value)}
      },
      highlight: {}
    });
  }

  archive.addEventListener('change', apply);
  title.addEventListener('input', apply);
  from.addEventListener('change', apply);
  to.addEventListener('change', apply);
}
