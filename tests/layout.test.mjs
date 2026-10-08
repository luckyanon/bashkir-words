import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createLexicon } from '../src/dictionary-service.mjs';
import { createLevelProvider, getRackCategory } from '../src/level-generator.mjs';
import { createGame, HINT_COST } from '../src/game.mjs';
import { layoutOptionsForSave } from '../src/layout-migration.mjs';
import { getWordCells, validateLevel } from '../src/levels.mjs';

const load = async name => JSON.parse(await readFile(new URL(`../public/data/${name}`, import.meta.url), 'utf8'));
const [raw, full, pools] = await Promise.all(['game-lexicon.nouns.json', 'nouns.full.json', 'level-pools.json'].map(load));
const lexicon = createLexicon(raw, full);
const identities = level => level.words.map(({ word, entryId }) => ({ word, entryId }))
  .sort((a, b) => a.word.localeCompare(b.word));

test('every noun pool creates a valid compact board without changing targets, bonuses or rack order', () => {
  const representatives = [5, 6, 7, 8].map(category => pools.pools.find(pool => Math.min(8, pool.letters.length) === category));
  for (const pool of pools.pools) {
    const category = Math.min(8, pool.letters.length);
    const data = { ...pools, pools: representatives.map(other => Math.min(8, other.letters.length) === category ? pool : other) };
    const id = Array.from({ length: 20 }, (_, index) => 121 + index).find(value => getRackCategory(value) === category);
    const legacy = createLevelProvider(data, lexicon, { layoutVersion: 1, applyLevelDifficulty:false }).getLevel(id);
    const compact = createLevelProvider(data, lexicon, {applyLevelDifficulty:false}).getLevel(id);
    const report = validateLevel(compact, lexicon);
    assert.equal(report.valid, true, `${pool.letters.join('')}: ${report.errors.join('; ')}`);
    assert.equal(compact.layoutVersion, 2);
    assert.deepEqual(identities(compact), identities(legacy));
    assert.deepEqual(compact.letters, legacy.letters);
    assert.deepEqual(compact.bonusWords, legacy.bonusWords);
    assert.equal(Math.max(...compact.words.map(placement => [...placement.word].length)), compact.letters.length);
    assert(compact.rows <= 24 && compact.cols <= 24);
  }
});

test('compact layout improves aggregate connectedness and area on identical campaign words', () => {
  const totals = { legacyComponents: 0, compactComponents: 0, legacyArea: 0, compactArea: 0, connected: 0, count: 0 };
  for (const seed of ['compact-regression-a', 'compact-regression-b', 'compact-regression-c']) {
    const legacy = createLevelProvider(pools, lexicon, { seed, layoutVersion: 1 });
    const compact = createLevelProvider(pools, lexicon, { seed });
    for (let id = 1001; id <= 1120; id++) {
      const before = legacy.getLevel(id), after = compact.getLevel(id);
      const beforeReport = validateLevel(before, lexicon), afterReport = validateLevel(after, lexicon);
      assert.equal(beforeReport.valid, true, beforeReport.errors.join('; '));
      assert.equal(afterReport.valid, true, afterReport.errors.join('; '));
      assert.deepEqual(identities(after), identities(before));
      assert.deepEqual(after.letters, before.letters);
      totals.legacyComponents += beforeReport.stats.componentCount;
      totals.compactComponents += afterReport.stats.componentCount;
      totals.legacyArea += before.rows * before.cols;
      totals.compactArea += after.rows * after.cols;
      totals.connected += Number(afterReport.stats.componentCount === 1);
      totals.count++;
    }
  }
  // Aggregate thresholds allow different valid outlines and occasional detached words.
  assert(totals.compactComponents < totals.legacyComponents * .8, JSON.stringify(totals));
  assert(totals.compactArea < totals.legacyArea * .98, JSON.stringify(totals));
  assert(totals.connected > totals.count * .35, JSON.stringify(totals));
});

test('layouts remain deterministic after cache eviction and preserve legacy coordinates through the boundary', () => {
  const seed = 'layout-cache-preservation';
  const legacy = createLevelProvider(pools, lexicon, { seed, layoutVersion: 1 });
  const upgraded = createLevelProvider(pools, lexicon, { seed, legacyLayoutThroughLevel: 40 });
  for (const id of [1, 19, 20, 39, 40]) assert.deepEqual(upgraded.getLevel(id), legacy.getLevel(id));
  const firstNew = upgraded.getLevel(41);
  assert.equal(firstNew.layoutVersion, 2);
  for (let id = 42; id <= 51; id++) upgraded.getLevel(id);
  assert.deepEqual(upgraded.getLevel(41), firstNew);
  assert.deepEqual(upgraded.getLevel(41), createLevelProvider(pools, lexicon, { seed }).getLevel(41));
  assert.deepEqual(upgraded.getLevel(40), legacy.getLevel(40));
});

test('layout migration freezes all previously unlocked IDs and keeps its boundary on subsequent reloads', () => {
  assert.deepEqual(layoutOptionsForSave(null), { layoutVersion: 2, legacyLayoutThroughLevel: 0 });
  assert.deepEqual(layoutOptionsForSave({ generatorVersion: 5, currentLevel: 40 }), { layoutVersion: 2, legacyLayoutThroughLevel: 0 });
  assert.deepEqual(layoutOptionsForSave({ generatorVersion: 6, currentLevel: 3, maxUnlocked: 40, completedRanges: [[1, 39]] }),
    { layoutVersion: 2, legacyLayoutThroughLevel: 40 });
  assert.deepEqual(layoutOptionsForSave({ generatorVersion: 6, currentLevel: 3, maxUnlocked: 1, completedRanges: [[1, 49]] }),
    { layoutVersion: 2, legacyLayoutThroughLevel: 50 });
  assert.deepEqual(layoutOptionsForSave({ generatorVersion: 6, layoutVersion: 2, currentLevel: 90, maxUnlocked: 90, legacyLayoutThroughLevel: 40 }),
    { layoutVersion: 2, legacyLayoutThroughLevel: 40 });
});

test('a partially solved old board retains exact hint cells, rewards and settings; only the next level is compact', () => {
  const seed = 'layout-save-preservation';
  const original = createLevelProvider(pools, lexicon, { seed, layoutVersion: 1 });
  const game = createGame(original, lexicon, null, { random: () => .7, now: () => new Date(2026, 9, 7, 14) });
  for (let id = 1; id < 40; id++) {
    assert.equal(game.startLevel(id), true);
    for (const placement of game.getCurrentLevel().words) game.submit(placement.word);
    assert.equal(game.getLevelState().completed, true);
  }
  assert.equal(game.startLevel(40), true);
  game.setSettings({ sound: false, animations: false });
  game.claimDailyGift();
  game.submit(game.getCurrentLevel().words[0].word);
  const hiddenBeforeHint = game.getHiddenCells();
  const coinsBeforeHint = game.state.coins;
  const hint = game.hint();
  assert.equal(hint.type, 'hint');
  assert.equal(game.state.coins, coinsBeforeHint - HINT_COST);
  assert(hiddenBeforeHint.some(cell => cell.key === hint.key));
  const beforeBoard = game.getBoard();
  const beforeProgress = structuredClone(game.getLevelState());
  assert.equal(beforeProgress.completed, false);
  // Simulate a real save from before layout metadata was introduced.
  const saved = game.snapshot();
  delete saved.layoutVersion;
  delete saved.legacyLayoutThroughLevel;
  const options = layoutOptionsForSave(saved);
  const upgraded = createLevelProvider(pools, lexicon, { seed, ...options });
  const restored = createGame(upgraded, lexicon, saved);
  assert.deepEqual(restored.getBoard(), beforeBoard);
  assert.deepEqual(restored.getLevelState(), beforeProgress);
  assert(restored.getLevelState().revealed.includes(hint.key));
  assert.deepEqual(restored.snapshot(), { ...saved, layoutVersion: 2, legacyLayoutThroughLevel: 40 });
  assert.deepEqual(createGame(createLevelProvider(pools, lexicon, { seed, ...layoutOptionsForSave(restored.snapshot()) }), lexicon, restored.snapshot()).snapshot(), restored.snapshot());
  for (const placement of restored.getCurrentLevel().words) restored.submit(placement.word);
  assert.equal(restored.state.maxUnlocked, 41);
  assert.equal(restored.startLevel(41), true);
  assert.equal(restored.getCurrentLevel().layoutVersion, 2);
  assert.deepEqual(restored.getCurrentLevel(), createLevelProvider(pools, lexicon, { seed }).getLevel(41));
  assert.deepEqual(restored.getLevel(40), original.getLevel(40));
});

test('crossing words still reveal their shared cells and cannot grant the completion reward twice', () => {
  const provider = createLevelProvider(pools, lexicon, { seed: 'compact-play-rules' });
  const game = createGame(provider, lexicon);
  const level = game.getCurrentLevel();
  const first = level.words[0];
  const result = game.submit(first.word);
  assert.equal(result.type, 'target');
  for (const cell of getWordCells(first)) assert(game.getLevelState().revealed.includes(cell.key));
  const beforeRepeat = game.snapshot();
  assert.equal(game.submit(first.word).type, 'repeat');
  assert.deepEqual(game.snapshot(), beforeRepeat);
  for (const placement of level.words) game.submit(placement.word);
  assert.equal(game.getLevelState().completed, true);
  const rewarded = game.snapshot();
  assert.equal(game.submit(first.word).reward, 0);
  assert.deepEqual(game.snapshot(), rewarded);
  assert.equal(game.replay(), true);
  for (const placement of level.words) game.submit(placement.word);
  assert.equal(game.state.coins, rewarded.coins);
});

