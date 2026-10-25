import {defineConfig} from 'vite';

// The viewer and session addresses in ../mosaica/mosaica.toml.
const viewer = process.env.MOSAICA_VIEWER_URL ?? 'http://127.0.0.1:9141';
const session = process.env.MOSAICA_SESSION_URL ?? 'http://127.0.0.1:9142';

export default defineConfig({
  server: {
    proxy: {
      '/mosaica': {target: viewer, rewrite: (path) => path.replace(/^\/mosaica/, '')}
    }
  },
  plugins: [
    {
      // Development only: every token this hands out sees every place.
      name: 'mosaica-dev-token',
      configureServer(server) {
        server.middlewares.use('/dev/token', async (_req, res) => {
          try {
            const answer = await fetch(`${session}/session/authorise`, {
              method: 'POST',
              headers: {authorization: `Bearer ${process.env.MOSAICA_CREDENTIAL}`, 'content-type': 'application/json'},
              body: JSON.stringify({read_all: true})
            });
            res.statusCode = answer.status;
            res.setHeader('content-type', 'application/json');
            res.end(await answer.text());
          } catch (error) {
            res.statusCode = 502;
            res.end(`Mosaica's session address did not answer: ${error}`);
          }
        });
      }
    }
  ]
});
