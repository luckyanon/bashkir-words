import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createLexicon, loadLexicon } from '../src/dictionary-service.mjs';
import { createGame, sanitizeGameState, HINT_COST, BONUS_REWARD, LEVEL_REWARD, INITIAL_COINS, DAILY_GIFT_REWARD } from '../src/game.mjs';

const {raw,full,starter}=JSON.parse(await readFile(new URL('./rules-fixture.json',import.meta.url),'utf8'));
const lexicon = createLexicon(raw, full, starter);
const placement = (word, row, col, direction) => ({ word, row, col, direction, entryId: lexicon.getEntry(word).id });
const levels = [
  { id: 1, title: 'Пересечения', letters: ['Б', 'А', 'Л', 'М', 'А'], rows: 2, cols: 6, words: [
    placement('АЛМА', 0, 2, 'across'), placement('БАЛ', 1, 0, 'across'), placement('АЛ', 0, 2, 'down'),
  ], bonusWords: ['АЛА'] },
  { id: 2, title: 'Новый уровень', letters: ['Ҡ', 'А', 'Л', 'А'], rows: 2, cols: 4, words: [
    placement('ҠАЛА', 0, 0, 'across'), placement('АҠ', 0, 1, 'down'),
  ] },
];

test('local lexicon preserves stable entries and only original Russian meanings', () => {
  const original = full.entries.find(entry => entry.display === 'АЛМА');
  assert.equal(lexicon.getEntry('а́лма'), original);
  assert.equal(lexicon.getEntryById(original.id), original);
  assert.deepEqual(lexicon.getMeanings('АЛМА').map(sense => sense.text), ['яблоко (яблочный)', 'яблоня (яблоневый; яблонный)']);
  assert.equal(lexicon.targetWords.length, 6);
  assert(lexicon.starterWords.length === 3);
  assert.deepEqual(lexicon.getMeanings('АЛА'), []);
  assert.notEqual(lexicon.normalize('Ө'), lexicon.normalize('О'));
  assert.notEqual(lexicon.normalize('Ҡ'), lexicon.normalize('К'));
});

test('loadLexicon requests only the two local noun resources', async () => {
  const seen = [];
  const resources = { 'game-lexicon.nouns.json': {...raw,starterWords:starter.starterWords}, 'nouns.full.json': full };
  const loaded = await loadLexicon('./assets/', async url => {
    seen.push(url);
    return { ok: true, json: async () => resources[url.split('/').at(-1)] };
  });
  assert.deepEqual(seen.sort(), ['game-lexicon.nouns.json', 'nouns.full.json'].map(name => `./assets/data/${name}`));
  assert.equal(loaded.dictionary.size, 7);
});

test('submitting crossing targets solves the third word and completes/unlocks once', () => {
  const game = createGame(levels, lexicon);
  assert.equal(game.startLevel(2), false);
  const first = game.submit('алма');
  assert.equal(first.type, 'target');
  assert.deepEqual(first.newlySolved, ['АЛМА']);
  assert.equal(game.getLevelState().revealed.includes('0,2'), true);
  const finish = game.submit('БАЛ');
  assert.equal(finish.completed, true);
  assert.deepEqual(finish.newlySolved, ['БАЛ', 'АЛ']);
  assert.equal(finish.completionReward, LEVEL_REWARD);
  assert.equal(game.state.coins, INITIAL_COINS + LEVEL_REWARD);
  assert.deepEqual(game.state.unlocked, [1, 2]);
  assert.equal(game.submit('АЛ').type, 'repeat');
  assert.equal(game.submit('БАЛ').reward, 0);
  assert.equal(game.state.coins, INITIAL_COINS + LEVEL_REWARD);
});

test('bonus without Russian translation is accepted and rewards only once', () => {
  const game = createGame(levels, lexicon);
  assert.equal(game.submit('АЛА').type, 'bonus');
  assert.equal(game.state.coins, INITIAL_COINS + BONUS_REWARD);
  assert.equal(game.submit('ала').type, 'repeat');
  assert.equal(game.submit('АЛА').reward, 0);
  assert.equal(game.state.coins, INITIAL_COINS + BONUS_REWARD);
  assert.deepEqual(game.state.bonusRewards, ['1:АЛА']);
});

test('found words mix targets and bonuses chronologically and retain order across reloads', () => {
  const game = createGame(levels, lexicon);
  game.submit('АЛМА');
  game.submit('АЛА');
  assert.deepEqual(game.getFoundWords(), ['АЛМА', 'АЛА']);
  const restored = createGame(levels, lexicon, game.snapshot());
  restored.submit('БАЛ');
  // The crossing finishes in the same move; the entered word stays last.
  assert.deepEqual(restored.getFoundWords(), ['АЛМА', 'АЛА', 'АЛ', 'БАЛ']);
  const snapshot = restored.snapshot();
  assert.deepEqual(createGame(levels, lexicon, snapshot).getFoundWords(), restored.getFoundWords());
  const copy = restored.getFoundWords(); copy.reverse();
  restored.submit('АЛА'); restored.submit('БАЛ'); restored.submit('ЛА'); restored.shuffle();
  assert.deepEqual(restored.snapshot(), snapshot);
  restored.replay(1);
  assert.deepEqual(restored.getFoundWords(), []);
  restored.submit('АЛА'); restored.submit('БАЛ');
  assert.deepEqual(restored.getFoundWords(), ['АЛА', 'БАЛ']);
  restored.reset();
  assert.deepEqual(restored.getFoundWords(), []);
});

test('a target completed by a paid hint is appended to discovery history', () => {
  const game = createGame([levels[1]], lexicon);
  game.submit('ҠАЛА');
  assert.deepEqual(game.getFoundWords(), ['ҠАЛА']);
  game.hint();
  assert.deepEqual(game.getFoundWords(), ['ҠАЛА', 'АҠ']);
  assert.deepEqual(createGame([levels[1]], lexicon, game.snapshot()).getFoundWords(), ['ҠАЛА', 'АҠ']);
});

test('legacy and damaged discovery history cannot invent found words or bonus rewards', () => {
  const game = createGame(levels, lexicon);
  game.submit('АЛА'); game.submit('АЛМА');
  const legacy = game.snapshot();
  delete legacy.levels[1].foundOrder;
  assert.deepEqual(createGame(levels, lexicon, legacy).getFoundWords(), ['АЛМА', 'АЛА']);
  const damaged = game.snapshot();
  damaged.levels[1].foundOrder = ['ала', 'АЛА', null, {}, 'БАЛ', 'НЕСУЩЕСТВУЕТ'];
  const restored = createGame(levels, lexicon, damaged);
  assert.deepEqual(restored.getFoundWords(), ['АЛА', 'АЛМА']);
  assert.equal(restored.state.coins, game.state.coins);
  assert.deepEqual(restored.getLevelState().solved, ['АЛМА']);
  damaged.bonusRanges = [[], [], []];
  assert.deepEqual(createGame(levels, lexicon, damaged).getFoundWords(), ['АЛМА']);
});

test('tile multiplicity, invalid words and shuffle preserve dictionary rules', () => {
  const game = createGame(levels, lexicon);
  assert.equal(lexicon.canBuild('АЛМА', ['А', 'Л', 'М']), false);
  assert.equal(lexicon.canBuild('АЛМА', levels[0].letters), true);
  assert.equal(game.submit('АТ').type, 'unbuildable');
  assert.equal(game.submit('ЛА').type, 'invalid');
  assert.equal(game.state.coins, INITIAL_COINS);
  for (let index = 0; index < 15; index++) assert.deepEqual(game.shuffle().sort(), [...levels[0].letters].sort());
});

test('hints reveal one hidden cell, derive crossing completions and charge exactly once', () => {
  const game = createGame(levels, lexicon);
  game.submit('АЛМА');
  game.submit('БАЛ');
  assert.equal(game.hint().type, 'complete');
  const fresh = createGame([levels[1]], lexicon);
  fresh.submit('ҠАЛА');
  const hint = fresh.hint();
  assert.equal(hint.type, 'hint');
  assert.equal(hint.key, '1,1');
  assert.equal(hint.cost, HINT_COST);
  assert.deepEqual(hint.newlySolved, ['АҠ']);
  assert.equal(hint.completed, true);
  assert.equal(fresh.state.coins, INITIAL_COINS - HINT_COST + LEVEL_REWARD);
  assert.equal(fresh.hint().type, 'complete');
});

test('insufficient funds cannot reveal cells or add hint events', () => {
  const snapshot = createGame(levels, lexicon).snapshot();
  snapshot.coins = HINT_COST - 1;
  const game = createGame(levels, lexicon, snapshot);
  assert.equal(game.hint().type, 'insufficient');
  assert.deepEqual(game.getLevelState().revealed, []);
  assert.equal(game.state.hintCount, 0);
  assert.equal(game.state.coins, HINT_COST - 1);
});

test('save restoration derives solved words and retains settings, bonus rewards and hint cost', () => {
  const game = createGame(levels, lexicon);
  game.setSettings({ sound: false, animations: false });
  game.submit('АЛА');
  const hint = game.hint();
  const saved = game.snapshot();
  const restored = createGame(levels, lexicon, saved);
  assert.deepEqual(restored.snapshot(), saved);
  assert(restored.getLevelState().revealed.includes(hint.key));
  assert.equal(restored.submit('АЛА').type, 'repeat');
  assert.equal(restored.state.coins, INITIAL_COINS + BONUS_REWARD - HINT_COST);
});

test('replays and restoring a replay cannot farm level or bonus rewards', () => {
  const game = createGame(levels, lexicon);
  game.submit('АЛА');
  game.submit('АЛМА');
  game.submit('БАЛ');
  const coins = game.state.coins;
  game.replay(1);
  assert.equal(game.getLevelState().completed, false);
  const restored = createGame(levels, lexicon, game.snapshot());
  assert(restored.state.unlocked.includes(2));
  assert.equal(restored.submit('АЛА').reward, 0);
  restored.submit('АЛМА');
  assert.equal(restored.submit('БАЛ').completionReward, 0);
  assert.equal(restored.state.coins, coins);
});

test('damaged saves reject version, inflated coins, bogus cells/bonuses and forged unlocks', () => {
  assert.equal(createGame(levels, lexicon, { version: 999, coins: 1e12 }).state.coins, INITIAL_COINS);
  const saved = createGame(levels, lexicon).snapshot();
  saved.coins = 1e12;
  saved.currentLevel = 2;
  saved.maxUnlocked = 999;
  saved.levels[1].solved = ['АЛМА', 'БАЛ', 'АЛ'];
  saved.levels[1].revealed = ['999,999', '__proto__'];
  saved.levels[1].rewarded = true;
  saved.levels[1].completed = true;
  saved.bonusRanges = [[[999, 999]], [[2, 2]], [[2, 2]]];
  const state = sanitizeGameState(levels, lexicon, saved);
  assert.equal(state.coins, INITIAL_COINS);
  assert.equal(state.currentLevel, 1);
  assert.deepEqual(state.unlocked, [1]);
  assert.deepEqual(state.levels[1].solved, []);
  assert.deepEqual(state.bonusRewards, []);
  assert.equal(state.levels[1].completed, false);
  assert.equal(state.levels[1].rewarded, false);
});

test('engine rejects a target outside the prepared lexicon or without a Russian meaning', () => {
  const missingMeaning = structuredClone(levels[0]);
  missingMeaning.words[0] = placement('АЛА', 0, 2, 'across');
  assert.throws(() => createGame([missingMeaning], lexicon), /Invalid level/);
});

test('hints sample the whole set of hidden cells and never select an open cell', () => {
  const first = createGame(levels, lexicon, null, { random: () => 0 });
  const last = createGame(levels, lexicon, null, { random: () => 0.999999 });
  const cells = first.getHiddenCells();
  assert.equal(first.hint().key, cells[0].key);
  assert.equal(last.hint().key, cells.at(-1).key);
  assert.notEqual(first.getLevelState().revealed[0], last.getLevelState().revealed[0]);
  const second = first.hint();
  assert.equal(second.key, cells[1].key);
  assert.equal(first.state.hintCount, 2);
  const restored = createGame(levels, lexicon, first.snapshot());
  assert.equal(restored.state.coins, INITIAL_COINS - HINT_COST * 2);
  assert.deepEqual(restored.getLevelState().revealed, first.getLevelState().revealed);
});

test('the same word on a later level is playable and bonuses reward once per level', () => {
  const repeated = [levels[0], { ...levels[0], id: 2 }];
  const game = createGame(repeated, lexicon);
  assert.equal(game.submit('АЛА').reward, BONUS_REWARD);
  game.submit('АЛМА'); game.submit('БАЛ');
  assert.equal(game.startLevel(2), true);
  assert.equal(game.submit('АЛА').type, 'bonus');
  assert.equal(game.submit('АЛМА').type, 'target');
  assert.equal(game.submit('АЛА').type, 'repeat');
  assert.equal(game.state.coins, INITIAL_COINS + LEVEL_REWARD + BONUS_REWARD * 2);
  game.submit('БАЛ');
  assert.equal(game.replay(1), true);
  const restored = createGame(repeated, lexicon, game.snapshot());
  assert.equal(restored.submit('АЛА').reward, 0);
  restored.submit('АЛМА');
  assert.equal(restored.submit('БАЛ').completionReward, 0);
});

test('valid dictionary words outside the declared bonus pool cannot add rewards', () => {
  const withoutBonus = createGame([{ ...levels[0], bonusWords: [] }], lexicon);
  assert.equal(withoutBonus.submit('АЛА').type, 'outside');
  assert.equal(withoutBonus.state.coins, INITIAL_COINS);
  assert.deepEqual(withoutBonus.getLevelState().bonus, []);
  assert.throws(() => createGame([{ ...levels[0], bonusWords: ['АЛА', 'МАЛ', 'АБА', 'БАЛА'] }], lexicon), /Invalid bonus/);
  assert.throws(() => createGame([{ ...levels[0], bonusWords: ['АЛМА'] }], lexicon), /Invalid bonus/);
});

test('shuffle changes visible order when possible even when random picks an identity permutation', () => {
  const game = createGame(levels, lexicon);
  const before = game.rack;
  const after = game.shuffle(() => 0.999999);
  assert.notDeepEqual(after, before);
  assert.deepEqual([...after].sort(), [...before].sort());
});

test('legacy saves preserve level number, unlocks, coins and preferences while resetting the board', () => {
  const provider = {
    seed: 'migration-seed', generatorVersion: 2, hasLevel: id => Number.isSafeInteger(id) && id >= 1,
    getLevel: id => ({ ...levels[0], id }),
  };
  const old = {
    version: 1, currentLevel: 8, unlocked: [1, 2, 3, 4, 5, 6, 7, 8, 9], coins: 217,
    settings: { sound: false, animations: false }, levels: { 8: { revealed: ['0,2'], solved: ['АЛМА'] } },
  };
  const game = createGame(provider, lexicon, old);
  assert.equal(game.state.currentLevel, 8);
  assert.equal(game.getMaxUnlocked(), 9);
  assert.equal(game.state.coins, 217);
  assert.deepEqual(game.state.settings, {...old.settings,music:true});
  assert.deepEqual(game.getLevelState().revealed, []);
  assert.equal(game.state.migration.boardReset, true);
  assert.deepEqual(game.state.legacy.levels, old.levels);
  const restored = createGame(provider, lexicon, game.snapshot());
  assert.deepEqual(restored.snapshot(), game.snapshot());
  assert.equal(restored.state.coins, 217);
});

test('lazy providers restore far progression without generating intervening levels or allocating unlock arrays', () => {
  const generated = [];
  const provider = { seed: 'lazy-test', generatorVersion: 2,
    getLevel: id => { generated.push(id); return { ...levels[0], id }; },
  };
  const save = createGame(provider, lexicon).snapshot();
  save.completedRanges = [[1, 10000]];
  save.maxUnlocked = 10001;
  save.currentLevel = 10001;
  save.levels = {};
  save.coins = INITIAL_COINS + LEVEL_REWARD * 10000;
  generated.length = 0;
  const game = createGame(provider, lexicon, save);
  assert.deepEqual(generated, [1, 10001]);
  assert.equal(game.getMaxUnlocked(), 10001);
  assert.equal(game.completedCount, 10000);
  assert.equal(game.startLevel(10002), false);
  game.submit('АЛМА'); game.submit('БАЛ');
  assert.equal(game.getMaxUnlocked(), 10002);
  assert.equal(game.startLevel(10002), true);
  assert.equal(game.getNextLevelId(), 10003);
  assert.equal(game.state.unlocked.length, 256);
  assert(JSON.stringify(game.snapshot()).length < 3000);
  assert.equal(game.getLevel(Number.MAX_SAFE_INTEGER), undefined);
  assert.equal(game.startLevel(Infinity), false);
  assert.equal(game.startLevel('invalid'), false);
});

test('generator updates retain active legacy boards, hints, rewards and preferences', () => {
  const legacyProvider = { seed: 'generator-update', generatorVersion: 2,
    getLevel: id => ({ ...levels[0], id }),
  };
  const original = createGame(legacyProvider, lexicon, null, { random: () => 0 });
  original.setSettings({ sound: false, animations: false });
  original.submit('АЛА');
  original.submit('АЛМА'); original.submit('БАЛ');
  original.startLevel(2);
  original.submit('АЛА');
  original.hint();
  const oldSave = original.snapshot();
  const provider = { ...legacyProvider, generatorVersion: 3, legacyThroughLevel: 2 };
  const updated = createGame(provider, lexicon, oldSave);
  assert.deepEqual(updated.snapshot(), { ...oldSave, generatorVersion: 3, legacyThroughLevel: 2 });
  assert.deepEqual(updated.getBoard(), original.getBoard());
  assert.deepEqual(updated.getLevelState(), original.getLevelState());
  assert.equal(updated.submit('АЛА').type, 'repeat');
  const restored = createGame(provider, lexicon, updated.snapshot());
  assert.deepEqual(restored.snapshot(), updated.snapshot());
  const coins = restored.state.coins;
  restored.submit('АЛМА'); restored.submit('БАЛ');
  assert.equal(restored.getMaxUnlocked(), 3);
  assert.equal(restored.state.legacyThroughLevel, 2);
  assert.equal(restored.state.coins, coins + LEVEL_REWARD);
});

test('generator updates require the same campaign seed and a preserved unlocked boundary', () => {
  const previous = { seed: 'old-generator', generatorVersion: 2, getLevel: id => ({ ...levels[0], id }) };
  const original = createGame(previous, lexicon);
  original.submit('АЛМА'); original.submit('БАЛ'); original.startLevel(2); original.hint();
  const saved = original.snapshot();
  for (const provider of [
    { ...previous, generatorVersion: 3, legacyThroughLevel: 1 },
    { ...previous, seed: 'different-generator', generatorVersion: 3, legacyThroughLevel: 2 },
  ]) {
    const updated = createGame(provider, lexicon, saved);
    assert.equal(updated.state.currentLevel, 1);
    assert.deepEqual(updated.getLevelState().revealed, []);
    assert.equal(updated.state.coins, INITIAL_COINS);
  }
});

test('daily gift grants 100 coins once and does not alter the active field', () => {
  let writes = 0;
  const game = createGame(levels, lexicon, null, { now: () => new Date(2026, 9, 7, 12).getTime(), onChange: () => writes++ });
  const field = structuredClone(game.getLevelState());
  assert.equal(game.getDailyGiftStatus().available, true);
  assert.deepEqual(game.claimDailyGift(), { type: 'gift', reward: DAILY_GIFT_REWARD });
  assert.equal(game.state.coins, INITIAL_COINS + 100);
  assert.deepEqual(game.getLevelState(), field);
  const after = game.snapshot();
  assert.deepEqual(game.claimDailyGift(), { type: 'gift-unavailable', reward: 0 });
  assert.deepEqual(game.snapshot(), after);
  assert.equal(writes, 1);
});

test('gift coins and daily lock survive reload together with hints, bonus and completion rewards', () => {
  const now = () => new Date(2026, 9, 7, 18).getTime();
  const game = createGame(levels, lexicon, null, { now, random: () => 0 });
  game.claimDailyGift();
  game.hint();
  game.submit('АЛА');
  game.submit('АЛМА');
  game.submit('БАЛ');
  const saved = game.snapshot();
  const restored = createGame(levels, lexicon, JSON.parse(JSON.stringify(saved)), { now });
  assert.equal(restored.state.coins, INITIAL_COINS + 100 - HINT_COST + BONUS_REWARD + LEVEL_REWARD);
  assert.deepEqual(restored.snapshot(), saved);
  assert.equal(restored.getDailyGiftStatus().available, false);
  assert.equal(restored.claimDailyGift().reward, 0);
});

test('gift countdown ends at local midnight and repeats are locked until the next day', () => {
  let time = new Date(2026, 9, 7, 12).getTime();
  const game = createGame(levels, lexicon, null, { now: () => time });
  game.claimDailyGift();
  assert.equal(game.getDailyGiftStatus().remainingMs, new Date(2026, 9, 8).getTime() - time);
  time = new Date(2026, 9, 7, 23, 59, 59, 999).getTime();
  const restored = createGame(levels, lexicon, game.snapshot(), { now: () => time });
  assert.equal(restored.getDailyGiftStatus().remainingMs, 1);
  assert.equal(restored.claimDailyGift().reward, 0);
  time += 1;
  assert.equal(restored.getDailyGiftStatus().available, true);
  assert.equal(restored.getDailyGiftStatus().remainingMs, 0);
  assert.equal(restored.claimDailyGift().reward, 100);
  time = new Date(2026, 9, 7, 12).getTime();
  assert.equal(restored.claimDailyGift().reward, 0);
  time = new Date(2026, 9, 12, 12).getTime();
  assert.equal(restored.claimDailyGift().reward, 100);
  assert.equal(restored.state.coins, INITIAL_COINS + 300);
});

test('timestamp gift saves migrate to the local claim date without losing coins', () => {
  const date = new Date(2026, 9, 7, 12);
  const initial = createGame(levels, lexicon).snapshot();
  initial.coins += 100;
  initial.dailyGift = {count: 1, lastClaimAt: date.getTime()};
  const game = createGame(levels, lexicon, initial, {now: () => date.getTime()});
  assert.equal(game.state.coins, INITIAL_COINS + 100);
  assert.deepEqual(game.state.dailyGift, {count: 1, lastClaimDay: Math.floor(Date.UTC(2026, 9, 7) / 86400000)});
  assert.equal(game.getDailyGiftStatus().available, false);
  const restored = createGame(levels, lexicon, game.snapshot(), {now: () => date.getTime()});
  assert.deepEqual(restored.snapshot(), game.snapshot());
});

test('older saves remain playable and malformed gift metadata cannot raise the coin limit', () => {
  const original = createGame(levels, lexicon);
  original.submit('АЛМА');
  const saved = original.snapshot();
  delete saved.dailyGift;
  const restored = createGame(levels, lexicon, saved);
  assert.equal(restored.state.coins, saved.coins);
  assert.deepEqual(restored.getLevelState(), saved.levels[1]);
  assert.equal(restored.getDailyGiftStatus().available, true);
  for (const dailyGift of [{count: -1, lastClaimDay: 20000}, {count: 1, lastClaimDay: null}, {count: '1', lastClaimDay: 20000}, {count: 40000, lastClaimDay: 20000}]) {
    const corrupt = createGame(levels, lexicon, { ...saved, coins: 90000, dailyGift });
    assert.equal(corrupt.state.coins, INITIAL_COINS);
    assert.equal(corrupt.state.dailyGift.count, 0);
  }
  const invalidClock = createGame(levels, lexicon, null, { now: () => NaN });
  assert.equal(invalidClock.claimDailyGift().reward, 0);
});

test('background motion switch persists independently of sound and animations',()=>{
 const game=createGame(levels,lexicon);game.setSettings({backgroundMotion:false});assert.equal(createGame(levels,lexicon,game.snapshot()).state.settings.backgroundMotion,false);game.setSettings({backgroundMotion:true});assert.equal(createGame(levels,lexicon,game.snapshot()).state.settings.backgroundMotion,true);game.setSettings({backgroundMotion:'false'});assert.equal(game.state.settings.backgroundMotion,true);
});
