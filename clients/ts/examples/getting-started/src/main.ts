// --8<-- [start:import]
import '@mosaicajs/components';
// --8<-- [end:import]

const explorer = document.querySelector('mosaica-explorer')!;

// --8<-- [start:token]
// Development only: the dev server hands out viewer tokens. Tutorial 3 shows how a real app gets them.
explorer.authorise = () => fetch('/dev/token').then((r) => r.json());
// --8<-- [end:token]
