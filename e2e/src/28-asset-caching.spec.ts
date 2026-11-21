import { expect, test } from '@playwright/test';

/**
 * Cache policy (D142). Trunk names the bundle after a content hash, so
 * a hashed name can never change meaning and may be cached forever.
 * Everything else — the app shell, `echarts.js` — keeps its name across
 * releases, so it must be revalidated: a browser that cached the shell
 * keeps pointing at the previous release's hashed bundle, and a new
 * app version looks like it never arrived. That is exactly what the
 * v0.15.0 history-page fix looked like on a real Home Assistant box.
 */
test('hashed bundles cache forever, stable names revalidate', async ({
  request,
}) => {
  const index = await request.get('./');
  expect(index.status()).toBe(200);
  expect(index.headers()['cache-control']).toBe('no-cache');

  const html = await index.text();
  const hashed = [...html.matchAll(/([\w.-]+-[0-9a-f]{16}(?:_bg)?\.\w+)/g)].map(
    (m) => m[1]!,
  );
  // The shell must reference at least the JS bundle and the stylesheet.
  expect(hashed.length).toBeGreaterThanOrEqual(2);

  for (const asset of new Set(hashed)) {
    const res = await request.get(asset);
    expect(res.status(), asset).toBe(200);
    expect(res.headers()['cache-control'], asset).toBe(
      'public, max-age=31536000, immutable',
    );
  }

  // A stable name from the same bundle: revalidated, never immutable.
  const echarts = await request.get('echarts.js');
  expect(echarts.status()).toBe(200);
  expect(echarts.headers()['cache-control']).toBe('no-cache');
});
