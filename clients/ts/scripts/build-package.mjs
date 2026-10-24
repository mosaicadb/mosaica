#!/usr/bin/env node
// Build the package in the working directory into `dist/`: JavaScript and declarations from
// `tsconfig.build.json`. The compiler writes to `dist.next/`, which replaces `dist/` only when it
// succeeds, so a deleted source file leaves nothing behind in a packed tarball and a failed build
// leaves the last good one in place. The components' single-file bundle, written into `dist/` by a
// later step, is carried over until that step replaces it.
//
// The build resolves the other workspace packages through their `dist/` declarations. A workspace
// dependency with no `dist/` is built first, so the package builds from a fresh clone.
import {execFileSync} from 'node:child_process';
import {existsSync, readFileSync, readdirSync, realpathSync, renameSync, rmSync} from 'node:fs';
import {createRequire} from 'node:module';
import {join} from 'node:path';

const manifest = JSON.parse(readFileSync('package.json', 'utf8'));
for (const name of Object.keys(manifest.dependencies ?? {}).filter((n) => n.startsWith('@mosaicajs/'))) {
  const dir = realpathSync(join('..', 'node_modules', name));
  if (!existsSync(join(dir, 'dist', 'index.d.ts'))) {
    console.log(`build-package: ${manifest.name} needs ${name}, which is not built; building it first`);
    execFileSync('npm', ['run', 'build', '--silent'], {cwd: dir, stdio: 'inherit'});
  }
}

const tsc = createRequire(import.meta.url).resolve('typescript/bin/tsc');
const next = 'dist.next';
rmSync(next, {recursive: true, force: true});
try {
  execFileSync(process.execPath, [tsc, '-p', 'tsconfig.build.json', '--outDir', next], {stdio: 'inherit'});
} catch {
  rmSync(next, {recursive: true, force: true});
  console.error(`build-package: ${manifest.name} did not compile; dist/ is unchanged`);
  process.exit(1);
}
if (existsSync('dist')) {
  for (const file of readdirSync('dist').filter((f) => f.startsWith('mosaica-components.'))) renameSync(join('dist', file), join(next, file));
  rmSync('dist', {recursive: true, force: true});
}
renameSync(next, 'dist');
