import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createLexicon } from '../src/dictionary-service.mjs';
import { createLevelProvider } from '../src/level-generator.mjs';
import { createHardcoreLayout } from '../src/hardcore-layout.mjs';
import { createHardcoreLayoutWithWasm, installHardcoreLayoutWasm } from '../src/hardcore-layout-wasm.mjs';

const readData = async file => JSON.parse(await readFile(new URL(`../public/data/${file}.json`, import.meta.url), 'utf8'));
const [pools, words, entries, binary] = await Promise.all([
  readData('level-pools'),
  readData('game-lexicon.nouns'),
  readData('nouns.full'),
  readFile(new URL('../public/wasm/hardcore-layout.wasm', import.meta.url)),
]);

// This matches the generator's seeded random stream. Keeping the JavaScript
// comparison independent lets this script detect differences in seed hashing.
function originalRandom(seed) {
  let state = 2166136261;
  for (const character of seed) state = Math.imul(state ^ character.codePointAt(0), 16777619) >>> 0;
  return () => {
    state += 0x6D2B79F5;
    let next = state;
    next = Math.imul(next ^ next >>> 15, next | 1);
    next ^= next + Math.imul(next ^ next >>> 7, next | 61);
    return ((next ^ next >>> 14) >>> 0) / 4294967296;
  };
}

const lexicon = createLexicon(words, entries);
const campaignSeed = 'benchmark-Башҡортса:hardcore';
const cases = pools.pools.filter(pool => pool.hardcoreWordCount >= 30).map((pool, index) => {
  const matches = lexicon.findWords(pool.letters, { minLength: 2, targetsOnly: true });
  assert.equal(matches.length, pool.hardcoreWordCount, `Rack ${pool.letters.join('')}`);
  return {
    rack: pool.letters.join(''),
    seed: `${campaignSeed}:hardcore:2:${index + 1}`,
    source: matches.map(word => ({ word, entryId: lexicon.getEntry(word).id })),
  };
});
const jsLayout = ({ source, seed }) => createHardcoreLayout(source, originalRandom(seed));
const wasmLayout = ({ source, seed }) => createHardcoreLayoutWithWasm(source, seed);
const sizeSorted = cases.toSorted((a, b) => a.source.length - b.source.length);
const layoutCases = Array.from({ length: 24 }, (_, index) =>
  sizeSorted[Math.round(index * (sizeSorted.length - 1) / 23)]);

// The sequence exceeds the provider's eight-entry cache.
const levelIds = [1, 2, 3, 10, 19, 20, 21, 35, 49, 50, 51, 61, 69, 70, 71, 85, 99, 100, 101, 150, 250, 500, 1000, 2500];
const createProvider = () => createLevelProvider(pools, lexicon, {
  seed: campaignSeed,
  hardcore: true,
});

function elapsedPerCase(items, run) {
  const start = performance.now();
  for (const item of items) run(item);
  return (performance.now() - start) / items.length;
}

async function benchmark(label, items, runJavaScript, runWasm, configure = async () => {}) {
  const samples = [[], []];
  const implementations = [runJavaScript, runWasm];
  for (let round = -2; round < 7; round++) {
    const order = round % 2 === 0 ? [0, 1] : [1, 0];
    for (const index of order) {
      await configure(index === 1);
      const elapsed = elapsedPerCase(items, implementations[index]);
      if (round >= 0) samples[index].push(elapsed);
    }
  }
  const [jsMedian, wasmMedian] = samples.map(values => values.toSorted((a, b) => a - b)[3]);
  console.log(`${label}: ${jsMedian.toFixed(2)} ms JS / ${wasmMedian.toFixed(2)} ms Wasm (${(jsMedian / wasmMedian).toFixed(2)}x)`);
}

const jsProvider = createProvider();
const wasmProvider = createProvider();
const originalLevels = levelIds.map(jsProvider.getLevel);
assert.equal(await installHardcoreLayoutWasm(binary), true);

for (const testCase of cases) {
  assert.deepEqual(wasmLayout(testCase), jsLayout(testCase), `Rack ${testCase.rack}`);
}
const wasmLevels = levelIds.map(wasmProvider.getLevel);
assert.deepEqual(wasmLevels, originalLevels, 'Generated levels');
console.log(`Parity: ${cases.length} dictionary racks and ${levelIds.length} complete levels`);

console.log(`Node ${process.version} · ${process.platform}/${process.arch}`);
console.log('Median of 7 batches, 2 warm-up batches, 24 cases per batch, alternating order');
const largestRack = sizeSorted.at(-1).source.length;
const levelCounts = originalLevels.map(level => level.words.length);
await benchmark(`Layout only (30–${largestRack} words)`, layoutCases, jsLayout, wasmLayout);
await benchmark(`Uncached getLevel (${Math.min(...levelCounts)}–${Math.max(...levelCounts)} words)`,
  levelIds, jsProvider.getLevel, wasmProvider.getLevel,
  enable => installHardcoreLayoutWasm(enable ? binary : null));
