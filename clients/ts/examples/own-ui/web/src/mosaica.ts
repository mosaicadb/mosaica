import {createStore, MosaicaClient} from '@mosaicajs/client';

const viewerUrl = 'http://localhost:9241';

export const client = new MosaicaClient({viewerUrl, sessionUrl: ''});
export const store = createStore({viewerUrl, client});
