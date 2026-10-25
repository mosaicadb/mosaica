import {mkdtemp, readFile, readdir, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {build} from 'vite';
import {expect, it} from 'vitest';

const root = join(import.meta.dirname, '..');

it('builds, with the explorer and its script in the page', async () => {
  const outDir = await mkdtemp(join(tmpdir(), 'mosaica-getting-started-'));
  try {
    await build({root, configFile: join(root, 'vite.config.ts'), logLevel: 'silent', build: {outDir, emptyOutDir: true}});
    const page = await readFile(join(outDir, 'index.html'), 'utf8');
    expect(page).toContain('<mosaica-explorer');
    const scripts = (await readdir(join(outDir, 'assets'))).filter((name) => name.endsWith('.js'));
    const code = (await Promise.all(scripts.map((name) => readFile(join(outDir, 'assets', name), 'utf8')))).join('\n');
    expect(code).toContain('mosaica-explorer');
    expect(code).toContain('/dev/token');
  } finally {
    await rm(outDir, {recursive: true, force: true});
  }
});
