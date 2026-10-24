const targets = [
  ['bing', 'https://www.bing.com/search?q=%E5%90%88%E6%88%90%E6%B5%8B%E8%AF%95&setmkt=zh-CN'],
  ['ddg-html', 'https://html.duckduckgo.com/html/?q=synthetic+test'],
];
const out = [];
for (const [name, url] of targets) {
  const t0 = Date.now();
  try {
    const res = await fetch(url, {
      headers: { 'user-agent': 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/120 Safari/537.36' },
      signal: AbortSignal.timeout(15000),
    });
    const body = await res.text();
    out.push({ engine: name, ok: res.ok, status: res.status, ms: Date.now() - t0, bytes: body.length,
               looksLikeResults: /result|结果|b_algo/i.test(body) });
  } catch (e) {
    out.push({ engine: name, ok: false, ms: Date.now() - t0, error: String(e).slice(0, 120) });
  }
}
console.log(JSON.stringify(out, null, 2));
