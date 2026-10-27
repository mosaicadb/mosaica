// Imported first by `basemap-page.ts`, before deck.gl, whose log may keep the console methods it
// finds when it loads: every error the page reports or logs, for the test to read.
export const errors: string[] = [];
const report = (e: unknown) => errors.push(String(e instanceof Error ? e.message : e));
addEventListener('error', (e) => report(e.error ?? e.message));
addEventListener('unhandledrejection', (e) => report(e.reason));
for (const method of ['error'] as const) {
  const original = console[method].bind(console);
  console[method] = (...args: unknown[]) => {
    report(args.map(String).join(' '));
    original(...args);
  };
}
