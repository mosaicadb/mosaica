import '@mosaicajs/components';
import './style.css';

const explorer = document.querySelector('mosaica-explorer')!;

// Development only: the dev server's tokens see every place.
explorer.authorise = async () => {
  const answer = await fetch('/dev/token');
  if (!answer.ok) throw new Error(await answer.text());
  const {token, expires_at} = await answer.json();
  return {token, expiresAt: expires_at};
};
