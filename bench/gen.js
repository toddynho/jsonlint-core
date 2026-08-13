// Generate realistic JSON fixtures: API-response-like payloads at several sizes
const fs = require('fs');

function rand(n) { return Math.floor(Math.random() * n); }
const words = ['alpha','beta','gamma','delta','node','parser','stream','token','buffer','index','value','schema','field','error','line','column'];
function str(len) {
  let s = [];
  while (s.join(' ').length < len) s.push(words[rand(words.length)]);
  return s.join(' ');
}

function record(i) {
  return {
    id: i,
    uuid: 'xxxxxxxx-xxxx-4xxx'.replace(/x/g, () => rand(16).toString(16)),
    name: str(20),
    active: i % 3 === 0,
    score: Math.random() * 1000,
    tags: Array.from({length: 5}, () => words[rand(words.length)]),
    nested: {
      created: '2026-08-13T12:00:00Z',
      meta: { depth: { level: 3, flags: [true, false, null] } },
      description: str(80)
    },
    nullable: i % 7 === 0 ? null : 'present'
  };
}

function build(targetBytes) {
  const records = [];
  let size = 0, i = 0;
  while (size < targetBytes) {
    const r = record(i++);
    size += JSON.stringify(r).length + 1;
    records.push(r);
  }
  return JSON.stringify({ status: 'ok', count: records.length, data: records });
}

for (const [name, bytes] of [['1kb', 1024], ['100kb', 100 * 1024], ['5mb', 5 * 1024 * 1024], ['25mb', 25 * 1024 * 1024]]) {
  const doc = build(bytes);
  fs.writeFileSync(`fixtures/${name}.json`, doc);
  console.log(name, (doc.length / 1024).toFixed(1) + ' KB');
}

// JSON5/JSONC variant of the 100kb doc (comments + trailing commas + unquoted keys stay valid JSON5)
const base = fs.readFileSync('fixtures/100kb.json', 'utf8');
const json5doc = '// config-style file with comments\n' + base.replace(/"status"/, '/* inline */ "status"');
fs.writeFileSync('fixtures/100kb.json5', json5doc);

// NDJSON for streaming test (~25MB)
const big = JSON.parse(fs.readFileSync('fixtures/25mb.json', 'utf8'));
fs.writeFileSync('fixtures/25mb.ndjson', big.data.map(r => JSON.stringify(r)).join('\n'));
console.log('fixtures written');
