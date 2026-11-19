import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createLexicon } from '../src/dictionary-service.mjs';
import { createGame } from '../src/game.mjs';
import { createLevelProvider } from '../src/level-generator.mjs';
import { createNativeGame } from '../src/native-game.mjs';

async function data(name) {
  return JSON.parse(await readFile(new URL(`../public/data/${name}.json`, import.meta.url), 'utf8'));
}

test('native bridge keeps the original renderer’s action and save contract', async () => {
  const [vocabulary, nouns, pools] = await Promise.all([
    data('game-lexicon.nouns'), data('nouns.full'), data('level-pools'),
  ]);
  const lexicon = createLexicon(vocabulary, nouns);
  const seed = 'native-classic-bridge';
  const levels = createLevelProvider(pools, lexicon, { seed });
  const hardcoreSource = createLevelProvider(pools, lexicon, { seed: `${seed}:hardcore`, hardcore: true });
  const now = () => new Date(2026, 10, 19, 9);
  const options = { hardcoreSource, now, random: () => 0.35 };
  const original = createGame(levels, lexicon, null, options);
  let stored;
  let engine;
  let writes = 0;
  const bridge = {
    dispatch(request) {
      const { op, word, mode, id, saved, localDay, sample, samples } = JSON.parse(request);
      const before = engine ? JSON.stringify(engine.snapshot()) : '';
      let result;
      switch (op) {
        case 'init':
          assert.equal(saved.campaignSeed, seed);
          engine = createGame(levels, lexicon, saved, options);
          break;
        case 'submit': {
          const response = engine.submit(word);
          result = { kind: response.type, word: response.word, reward: response.completionReward || response.reward };
          break;
        }
        case 'hint': {
          assert.equal(sample, 0.35);
          const response = engine.hint();
          result = { kind: response.type, reward: response.completionReward || 0 };
          break;
        }
        case 'shuffle':
          assert.deepEqual(samples, Array(engine.rack.length - 1).fill(0.35));
          engine.shuffle();
          break;
        case 'claimDailyGift':
          assert.equal(engine.getDailyGiftStatus().day, localDay);
          result = engine.claimDailyGift().type === 'gift';
          break;
        case 'setMode':
          result = engine.setMode(mode);
          break;
        case 'startLevel':
          result = engine.startLevel(id);
          break;
        case 'regenerateHardcore':
          result = engine.regenerateHardcore();
          break;
        default:
          throw new Error(`Unexpected operation ${op}`);
      }
      const state = engine.snapshot();
      const level = { mode: 'classic', ...engine.getCurrentLevel() };
      return JSON.stringify({ ok: true, state, level, rack: engine.rack, result, changed: JSON.stringify(state) !== before });
    },
  };
  const adapter = createNativeGame(levels, lexicon, null, { ...options, onChange: state => { stored = state; writes++; } }, bridge);

  assert.equal(writes, 0);
  assert.deepEqual(adapter.getBoard(), original.getBoard());
  assert.deepEqual(adapter.getDailyGiftStatus(), original.getDailyGiftStatus());
  assert.deepEqual(adapter.claimDailyGift(), original.claimDailyGift());
  assert.deepEqual(adapter.claimDailyGift(), original.claimDailyGift());
  assert.equal(writes, 1);
  assert.deepEqual(adapter.hint(), original.hint());
  assert.equal(writes, 2);
  const word = original.getCurrentLevel().words.find(entry => !original.getSolvedWords().includes(entry.word)).word;
  assert.deepEqual(adapter.submit(word), original.submit(word));
  assert.deepEqual(adapter.submit(word), original.submit(word));
  assert.equal(writes, 3);
  assert.deepEqual(adapter.getFoundWords(), original.getFoundWords());

  assert.deepEqual(adapter.shuffle(), original.shuffle());
  assert.equal(writes, 3);
  assert.equal(adapter.setMode('hardcore'), original.setMode('hardcore'));
  assert.deepEqual(adapter.getBoard(), original.getBoard());
  assert.equal(adapter.regenerateHardcore(), original.regenerateHardcore());
  assert.deepEqual(adapter.getBoard(), original.getBoard());
  assert.deepEqual(adapter.getLevelState(), original.getLevelState());
  assert.equal(adapter.startLevel(1), original.startLevel(1));
  assert.deepEqual(adapter.getLevelState(1), original.getLevelState(1));
  assert.deepEqual(adapter.snapshot(), original.snapshot());
  assert.deepEqual(stored, adapter.snapshot());
});
