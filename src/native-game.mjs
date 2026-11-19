import { availableBackground, BACKGROUNDS } from './backgrounds.mjs';
import { createGame, DAILY_GIFT_REWARD, HINT_COST } from './game.mjs';
import { buildBoard } from './levels.mjs';

const DAY_MS = 86_400_000;
const MAX_LEVEL_ID = Number.MAX_SAFE_INTEGER - 1;
const copy = value => JSON.parse(JSON.stringify(value));
const levelId = id => Number.isSafeInteger(id) && id >= 1 && id <= MAX_LEVEL_ID;
function sample(random) {
  const value = Number(random());
  return Number.isFinite(value) ? value : 0;
}

function localDay(now) {
  const date = new Date(now);
  if (!Number.isFinite(date.getTime())) return null;
  return Math.floor(Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()) / DAY_MS);
}

// The original renderer owns browser timing and presentation. Android supplies
// the authoritative game state, rack and level for each completed operation.
export function createNativeGame(levelSource, lexicon, saved, options = {}, bridge) {
  const original = createGame(levelSource, lexicon, saved, options);
  if (typeof bridge?.dispatch !== 'function') return original;

  let state, level, board, rack;
  const readTime = () => new Date(options.now ? options.now() : Date.now()).getTime();

  function call(op, fields = {}) {
    const nowMillis = readTime();
    const reply = JSON.parse(bridge.dispatch(JSON.stringify({ op, nowMillis, localDay: localDay(nowMillis), ...fields })));
    if (!reply?.ok || !reply.state || !reply.level || !Array.isArray(reply.rack)) {
      throw new Error(reply?.error || 'Invalid response from the Android game engine');
    }
    if (!level || op === 'reset' || reply.level.id !== level.id || reply.level.mode !== level.mode) {
      level = reply.level;
      board = buildBoard(level);
    }
    state = reply.state;
    rack = reply.rack;
    if (reply.changed && op !== 'init') options.onChange?.(copy(state));
    return reply.result;
  }

  call('init', { saved: original.snapshot(), seed: levelSource.seed });

  const isHardcore = () => state.mode === 'hardcore';
  function getLevel(id) {
    if (id === undefined || (!isHardcore() && Number(id) === level.id)) return level;
    const number = Number(id);
    return levelId(number) && levelSource.hasLevel(number) ? levelSource.getLevel(number) : undefined;
  }
  function getBoard(id) {
    const selected = getLevel(id);
    return selected === level ? board : selected ? buildBoard(selected) : [];
  }
  function getLevelState(id) {
    if (id === undefined && isHardcore()) return state.hardcore.progress;
    const savedLevel = state.levels[id ?? state.currentLevel];
    if (savedLevel) return savedLevel;

    // The original API also exposes older classic boards, including boards the
    // save no longer caches. Reconstruct that read without mutating Android state.
    return createGame(levelSource, lexicon, state, { hardcoreSource: options.hardcoreSource }).getLevelState(id);
  }

  function outcome(before, completionReward) {
    const after = getLevelState();
    const solvedBefore = new Set(before.solved);
    return {
      newlySolved: after.solved.filter(word => !solvedBefore.has(word)),
      completed: after.completed,
      completionReward,
    };
  }

  function submit(value) {
    const before = getLevelState();
    const { kind: type, word, reward } = call('submit', { word: value });
    if (type === 'target') return { type, word, reward: 0, ...outcome(before, reward) };
    if (type === 'bonus') {
      return { type, word, reward, completed: before.completed, newlySolved: [], completionReward: 0 };
    }
    if (type === 'repeat') {
      return { type, word, bonus: before.bonus.includes(word), reward: 0, completed: before.completed };
    }
    if (type === 'excluded-length') return { type, word, length: [...word].length, reward: 0 };
    return { type, word, reward: 0 };
  }

  function hint() {
    const before = getLevelState();
    const fields = !isHardcore() && state.coins >= HINT_COST && getHiddenCells().length
      ? { sample: sample(options.random || Math.random) }
      : {};
    const { kind: type, reward } = call('hint', fields);
    if (type !== 'hint') return { type, cost: type === 'unavailable' ? 0 : HINT_COST };
    const shownBefore = new Set(before.revealed);
    const key = getLevelState().revealed.find(cell => !shownBefore.has(cell));
    return { type, key, cost: HINT_COST, ...outcome(before, reward) };
  }

  function getHiddenCells() {
    const shown = new Set(getLevelState().revealed);
    return board.filter(cell => !shown.has(cell.key));
  }

  function getDailyGiftStatus() {
    const now = readTime();
    const day = localDay(now);
    const valid = Number.isFinite(now) && now >= 0 && Number.isSafeInteger(day);
    const last = state.dailyGift.lastClaimDay;
    const available = valid && (last === null || day > last);
    let remainingMs = 0;
    if (valid && !available) {
      const date = new Date(last * DAY_MS);
      const midnight = new Date(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate() + 1).getTime();
      remainingMs = Math.max(0, midnight - now);
    }
    return { available, reward: DAILY_GIFT_REWARD, remainingMs, day };
  }

  return {
    get state() { return state; },
    get rack() { return [...rack]; },
    get completedCount() { return state.completedRanges.reduce((sum, [first, last]) => sum + last - first + 1, 0); },
    get isHardcore() { return isHardcore(); },
    getLevel,
    getCurrentLevel: () => level,
    getBoard,
    getLevelState,
    getSolvedWords: id => [...(getLevelState(id)?.solved || [])],
    getFoundWords: id => [...(getLevelState(id)?.foundOrder || [])],
    getHiddenCells,
    getMaxUnlocked: () => state.maxUnlocked,
    getNextLevelId: () => isHardcore() || level.id < MAX_LEVEL_ID ? level.id + 1 : undefined,
    startLevel: id => levelId(Number(id)) && call('startLevel', { id: Number(id) }),
    replay: (id = state.currentLevel) => levelId(Number(id)) && call('replay', { id: Number(id) }),
    setMode: mode => call('setMode', { mode }),
    regenerateHardcore: () => call('regenerateHardcore'),
    reset() { call('reset'); },
    shuffle(random = options.random || Math.random) {
      const samples = Array.from({ length: rack.length - 1 }, () => sample(random));
      call('shuffle', { samples });
      return [...rack];
    },
    submit,
    hint,
    setSettings(update = {}) {
      call('setSettings', { update });
      return { ...state.settings };
    },
    ownsBackground: id => !!availableBackground(id, state.backgrounds.owned),
    purchaseBackground(id) {
      const { kind: type } = call('purchaseBackground', { id });
      const background = BACKGROUNDS.find(entry => entry.id === id);
      if (type === 'invalid' || !background) return { type };
      if (type === 'insufficient') return { type, cost: background.price };
      return { type, background: id, cost: type === 'owned' ? 0 : background.price };
    },
    getDailyGiftStatus,
    claimDailyGift() {
      const claimed = call('claimDailyGift');
      return claimed ? { type: 'gift', reward: DAILY_GIFT_REWARD } : { type: 'gift-unavailable', reward: 0 };
    },
    snapshot: () => copy(state),
  };
}
