import '@mosaicajs/components';
import {filters} from './filters.ts';
import {store} from './mosaica.ts';
import {paper} from './paper.ts';
import {results} from './results.ts';
import {topics} from './topics.ts';
import './style.css';

document.querySelector('mosaica-store')!.store = store;
filters(store);
results(store);
paper(store);
topics(store);
