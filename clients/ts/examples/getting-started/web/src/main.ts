import '@mosaicajs/components';
import './style.css';

const explorer = document.querySelector('mosaica-explorer')!;

// The explorer calls this for a token when it starts, and again before each one expires.
explorer.authorise = async () => {
  const answer = await fetch('/dev/token');
  if (!answer.ok) throw new Error(await answer.text());
  const {token, expires_at} = await answer.json();
  return {token, expiresAt: expires_at};
};
