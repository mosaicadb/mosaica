import '@mosaicajs/components';
import {filters} from './filters.ts';
import {client, store} from './mosaica.ts';
import {paper} from './paper.ts';
import {results} from './results.ts';
import {topics} from './topics.ts';
import './style.css';

document.querySelector('mosaica-store')!.store = store;
filters(store);
results(store, client);
paper(store);
topics(store);
