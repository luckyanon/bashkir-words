import { canBuildWord, normalizeWord } from './lib/dictionary.mjs';
import { buildBoard, getWordCells, validateLevel } from './levels.mjs';
import {BACKGROUNDS,availableBackground} from './backgrounds.mjs';

export const SAVE_VERSION = 2;
export const INITIAL_COINS = 90;
export const HINT_COST = 20;
export const BONUS_REWARD = 5;
export const LEVEL_REWARD = 60;
export const HARDCORE_WORD_REWARD = 0;
export const HARDCORE_LEVEL_REWARD = 0;
export const DAILY_GIFT_REWARD = 100;
const MAX_LEVEL_ID = Number.MAX_SAFE_INTEGER - 1;
const RECENT_LEVEL_LIMIT = 64;
const clone = value => JSON.parse(JSON.stringify(value));
const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const unique = value => [...new Set(Array.isArray(value) ? value : [])];
const validId = value => Number.isSafeInteger(Number(value)) && Number(value) >= 1 && Number(value) <= MAX_LEVEL_ID;
const safeCount = value => Number.isSafeInteger(value) && value >= 0 ? value : 0;
const capped = value => Math.min(Number.MAX_SAFE_INTEGER, Math.max(0, value));
const emptyLevel = () => ({ revealed: [], solved: [], bonus: [], foundOrder: [], completed: false, rewarded: false });
const emptyHardcore = () => ({currentLevel:1,completedCount:0,bonusCount:0,wordCount:0,rewardRevision:2,rewardCoins:0,legacyEarnings:0,legacyCompletedCount:0,legacyBonusCount:0,rewardedWords:[],progress:emptyLevel()});
const legacyHardcoreEarnings=h=>h?capped(safeCount(h.legacyCompletedCount)*30+(safeCount(h.completedCount)-safeCount(h.legacyCompletedCount))*500+safeCount(h.legacyBonusCount)*BONUS_REWARD+(safeCount(h.bonusCount)-safeCount(h.legacyBonusCount)+safeCount(h.wordCount))):0;
const hardcoreEarnings=h=>h?capped(safeCount(h.legacyEarnings)+safeCount(h.rewardCoins)):0;
export const hardcoreCompletionReward=(words,bonus)=>words*6+bonus*12;
const emptyGift = () => ({ count: 0, lastClaimDay: null });
function localDay(value) {
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) return null;
  return Math.floor(Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()) / 86400000);
}
function sanitizeGift(gift) {
  if (!object(gift) || !Number.isSafeInteger(gift.count) || gift.count <= 0) return emptyGift();
  let day = gift.lastClaimDay;
  if (day === undefined && Number.isSafeInteger(gift.lastClaimAt) && gift.lastClaimAt >= 0 && gift.lastClaimAt <= 8640000000000000) day = localDay(gift.lastClaimAt);
  return Number.isSafeInteger(day) && day >= gift.count - 1 && day <= 100000000
    ? { count: gift.count, lastClaimDay: day } : emptyGift();
}

// Contiguous completion/reward intervals keep years of ordinary progression compact.
function contains(ranges, id) { return (ranges||[]).some(([first, last]) => id >= first && id <= last); }
function count(ranges) { return ranges.reduce((sum, [first, last]) => sum + last - first + 1, 0); }
function normalizeRanges(value, hasLevel) {
  const ranges = (Array.isArray(value) ? value : []).slice(0, 100000)
    .filter(range => Array.isArray(range) && range.length === 2 && validId(range[0]) && validId(range[1]) &&
      Number(range[0]) <= Number(range[1]) && hasLevel(Number(range[0])) && hasLevel(Number(range[1])))
    .map(([first, last]) => [Number(first), Number(last)]).sort((a, b) => a[0] - b[0]);
  const merged = [];
  for (const range of ranges) {
    const previous = merged.at(-1);
    if (previous && range[0] <= previous[1] + 1) previous[1] = Math.max(previous[1], range[1]);
    else merged.push(range);
  }
  return merged;
}
function insertRange(ranges, id) {
  if (contains(ranges, id)) return;
  const merged = normalizeRanges([...ranges, [id, id]], () => true);
  ranges.splice(0, ranges.length, ...merged);
}

export function classicBonusWords(level, lexicon) {
  const bonuses=unique((level.bonusWords||[]).map(normalizeWord));
  if(level.generatorVersion>=6 && level.mode!=='hardcore'){
    const targets=new Set(level.words.map(item=>item.word));
    const extras=lexicon.findWords(level.letters,{minLength:level.minWordLength,targetsOnly:true}).filter(word=>lexicon.isTargetCandidate(word)&&!targets.has(word)&&!bonuses.includes(word));
    while(bonuses.length<3)bonuses.push(null);
    bonuses.push(...extras);
  }
  return bonuses;
}
export function alignBonusWords(previous, current) {
  const available = new Set(current.filter(Boolean));
  return [...previous.map(word => available.has(word) ? word : null),
    ...current.filter(word => word && !previous.includes(word))];
}
function prepareSource(levelSource, lexicon) {
  const finite = Array.isArray(levelSource);
  if (finite && !levelSource.length) throw new TypeError('At least one level is required');
  if (!finite && typeof levelSource?.getLevel !== 'function') throw new TypeError('A level array or lazy provider is required');
  const index = finite ? new Map(levelSource.map(level => [Number(level.id), level])) : null;
  if (finite && (index.size !== levelSource.length || levelSource.some(level => !validId(level.id)))) throw new TypeError('Duplicate or invalid level ID');
  const ids = finite ? [...index.keys()] : null;
  const firstId = finite ? ids[0] : 1;
  const cache = new Map();
  const hasLevel = id => validId(id) && (finite ? index.has(Number(id)) : levelSource.hasLevel?.(Number(id)) !== false);
  function getMeta(value) {
    const id = Number(value);
    if (!hasLevel(id)) return null;
    if (cache.has(id)) return cache.get(id);
    const level = finite ? index.get(id) : levelSource.getLevel(id);
    if (!level || Number(level.id) !== id) throw new TypeError(`Invalid level ID ${id}`);
    const report = validateLevel(level, lexicon);
    if (!report.valid) throw new TypeError(`Invalid level ${id}: ${report.errors.join('; ')}`);
    const words = level.words.map(placement => ({ word: normalizeWord(placement.word), keys: getWordCells(placement).map(cell => cell.key) }));
    let bonuses = unique((Array.isArray(level.bonusWords) ? level.bonusWords : []).map(normalizeWord));
    const excludedLengths = Number(level.generatorVersion) >= 3 ? level.excludedWordLengths || (level.letters.length >= 7 ? [2, 3] : level.letters.length === 6 ? [2] : []) : [];
    if (bonuses.length > 3 || bonuses.some(word => !lexicon.hasWord(word) || !lexicon.canBuild(word, level.letters)
      || excludedLengths.includes([...word].length) || words.some(target => target.word === word))) {
      throw new TypeError(`Invalid bonus words in level ${id}`);
    }
    bonuses=classicBonusWords(level,lexicon);
    // Preserve old slot identities, including removed words as empty slots.
    // The old ledger remains compact and no migrated bonus can pay twice.
    if(levelSource.legacyBonusWords){
      const previous=levelSource.legacyBonusWords(id);
      if(previous)bonuses=alignBonusWords(previous,bonuses);
    }
    const meta = { id, level, board: report.cells || buildBoard(level), words, bonuses, excludedLengths };
    cache.set(id, meta);
    if (cache.size > RECENT_LEVEL_LIMIT) cache.delete(cache.keys().next().value);
    return meta;
  }
  function reachable(ranges) {
    if (finite) {
      let position = 0;
      while (position < ids.length - 1 && contains(ranges, ids[position])) position++;
      return ids[position];
    }
    const first = ranges.find(([start]) => start === 1);
    return Math.min(MAX_LEVEL_ID, first ? first[1] + 1 : 1);
  }
  const canStart = (id, unlocked) => hasLevel(id) && (finite ? ids.indexOf(Number(id)) <= ids.indexOf(unlocked) : Number(id) <= unlocked);
  const nextId = id => finite ? ids[ids.indexOf(id) + 1] : id < MAX_LEVEL_ID ? id + 1 : undefined;
  getMeta(firstId);
  return { getMeta, hasLevel, firstId, reachable, canStart, nextId, ids,
    bonusLayouts:clone(levelSource.bonusLayouts||[]),bonusLayoutThroughLevel:safeCount(levelSource.bonusLayoutThroughLevel),bonusSlotLimit:Math.max(3,(lexicon.dictionary?.size||lexicon.targetWords?.length||3)*2),rulesRevision:Number(levelSource.rulesRevision||0),seed: String(levelSource.seed ?? 'legacy-array'), generatorVersion: Number(levelSource.generatorVersion ?? 1),
    legacyThroughLevel: safeCount(levelSource.legacyThroughLevel), legacyV3ThroughLevel: safeCount(levelSource.legacyV3ThroughLevel),
    layoutVersion:Number(levelSource.layoutVersion ?? 1), legacyLayoutThroughLevel:safeCount(levelSource.legacyLayoutThroughLevel),datasetRevision:String(levelSource.datasetRevision||""),difficultyRevision:Number(levelSource.difficultyRevision||0),wordRevision:Number(levelSource.wordRevision||0) };
}

function orderedFoundWords(candidate, solved = candidate?.solved || [], bonus = candidate?.bonus || []) {
  const available = new Set([...solved, ...bonus]);
  const words = value => unique(value).filter(word => typeof word === 'string').map(normalizeWord);
  // Older saves have no shared chronology. Retain their existing display order
  // and append future discoveries to one history for targets and bonuses.
  return unique([...words(candidate?.foundOrder), ...words(candidate?.solved), ...solved, ...bonus])
    .filter(word => available.has(word));
}

function deriveLevel(meta, candidate, rewarded) {
  const keys = new Set(meta.board.map(cell => cell.key));
  const revealed = candidate ? unique(candidate.revealed).filter(key => typeof key === 'string' && keys.has(key)) : rewarded ? [...keys] : [];
  const shown = new Set(revealed);
  const solved = meta.words.filter(item => item.keys.every(key => shown.has(key))).map(item => item.word);
  const bonus = unique(candidate?.bonus).map(normalizeWord).filter(word => meta.bonuses.includes(word));
  return { revealed, solved, bonus, foundOrder: orderedFoundWords(candidate, solved, bonus), completed: solved.length === meta.words.length, rewarded };
}

function attachCompatibility(state, source) {
  // These bounded views support small existing integrations; use maxUnlocked for navigation.
  Object.defineProperties(state, {
    unlocked: { configurable: true, get: () => source.ids ? source.ids.filter(id => source.canStart(id, state.maxUnlocked)) :
      Array.from({ length: Math.min(state.maxUnlocked, 256) }, (_, index) => Math.max(1, state.maxUnlocked - 255) + index) },
    bonusRewards: { configurable: true, get: () => Object.keys(state.levels).flatMap(id => {
      const meta = source.getMeta(id);
      return meta.bonuses.filter((word, index) => word&&contains(state.bonusRanges[index], Number(id))).map(word => `${id}:${word}`);
    }) },
    hintLedger: { configurable: true, get: () => [] },
  });
  return state;
}

function sanitizePrepared(source, saved, hardcoreSource) {
  const state = {
    version: SAVE_VERSION, campaignSeed: source.seed, generatorVersion: source.generatorVersion,
    currentLevel: source.firstId, maxUnlocked: source.firstId, coins: INITIAL_COINS,
    settings: { sound: true, music: true, animations: true }, levels: {}, completedRanges: [], bonusRanges: [[], [], []], hintCount: 0,
    economy: { baseCoins: INITIAL_COINS, completedBaseline: 0, rewardRevision:2, legacyClassicCompletions:0 },
    dailyGift: emptyGift(),
    backgrounds: {version:1,owned:['ufa'],paid:[]},
  };
  if (source.generatorVersion >= 3) state.legacyThroughLevel = source.legacyThroughLevel;
  if (source.generatorVersion >= 4) state.legacyV3ThroughLevel = source.legacyV3ThroughLevel;
  if(source.generatorVersion>=6){state.layoutVersion=source.layoutVersion;state.legacyLayoutThroughLevel=source.legacyLayoutThroughLevel;state.datasetRevision=source.datasetRevision;state.wordRevision=source.wordRevision;state.difficultyRevision=source.difficultyRevision;}
  if(source.bonusLayoutThroughLevel)state.bonusLayoutThroughLevel=source.bonusLayoutThroughLevel;
  if(source.bonusLayouts.length)state.bonusLayouts=clone(source.bonusLayouts);
  const valid = object(saved) && object(saved.levels) && (saved.version === SAVE_VERSION || saved.version === 1);
  if(hardcoreSource){state.mode='classic';state.hardcore=emptyHardcore();state.hardcore.datasetRevision=source.datasetRevision;state.hardcore.rulesRevision=hardcoreSource.rulesRevision;}
  if (valid) for (const key of ['sound', 'music', 'animations', 'backgroundMotion']) if (typeof saved.settings?.[key] === 'boolean') state.settings[key] = saved.settings[key];
  if (valid && saved.version === 1) {
    const oldUnlocked = unique(saved.unlocked).map(Number).filter(id => validId(id) && id <= 32 && source.hasLevel(id));
    let max = source.firstId;
    for (const id of oldUnlocked.sort((a, b) => a - b)) if (id === max + 1 || id === max) max = id;
    state.currentLevel = oldUnlocked.includes(Number(saved.currentLevel)) ? Number(saved.currentLevel) : source.firstId;
    state.maxUnlocked = max;
    if (max > 1) state.completedRanges = [[1, max - 1]];
    state.coins = Number.isSafeInteger(saved.coins) && saved.coins >= 0 ? saved.coins : INITIAL_COINS;
    state.economy = { baseCoins: state.coins, completedBaseline: count(state.completedRanges),rewardRevision:2,legacyClassicCompletions:0 };
    state.legacy = { version: 1, currentLevel: saved.currentLevel, unlocked: oldUnlocked, coins: state.coins, levels: clone(saved.levels) };
    state.migration = { fromVersion: 1, boardReset: true, currentLevel: state.currentLevel,
      message: 'Игра обновлена: номер уровня, монеты и настройки сохранены. Текущее поле создано заново, чтобы на каждом уровне было не меньше 7 слов. Старый прогресс сохранён в резервной копии.' };
    state.levels[state.currentLevel] = deriveLevel(source.getMeta(state.currentLevel), emptyLevel(), contains(state.completedRanges, state.currentLevel));
    return attachCompatibility(state, source);
  }
  const sameGenerator = saved?.generatorVersion === source.generatorVersion
    && (source.generatorVersion < 3 || safeCount(saved.legacyThroughLevel) === source.legacyThroughLevel)
    && (source.generatorVersion < 4 || safeCount(saved.legacyV3ThroughLevel) === source.legacyV3ThroughLevel);
  // Existing boards keep the exact v2 generator through their last unlocked level.
  // Only future IDs enter the new weighted campaign, so revealed cells and bonus
  // reward indexes retain their meaning across the update.
  const previousCompletions = normalizeRanges(saved?.completedRanges, source.hasLevel);
  const upgradedGenerator = (saved?.generatorVersion === 2 && source.generatorVersion === 3
    && source.legacyThroughLevel >= source.reachable(previousCompletions)
    && previousCompletions.every(([, last]) => last <= source.legacyThroughLevel))
    || ([2, 3].includes(saved?.generatorVersion) && source.generatorVersion === 4
      && source.legacyV3ThroughLevel >= source.reachable(previousCompletions)
      && previousCompletions.every(([, last]) => last <= source.legacyV3ThroughLevel)
      && source.legacyThroughLevel === (saved.generatorVersion === 2 ? source.legacyV3ThroughLevel : safeCount(saved.legacyThroughLevel)));
  const sameLayout=(saved?.layoutVersion ?? 1)===source.layoutVersion && safeCount(saved?.legacyLayoutThroughLevel)===source.legacyLayoutThroughLevel;
  const preservedLayout=(saved?.layoutVersion ?? 1)===1 && source.layoutVersion===2
    && source.legacyLayoutThroughLevel>=source.reachable(previousCompletions)
    && previousCompletions.every(([,last])=>last<=source.legacyLayoutThroughLevel);
  const layoutCompatible=source.generatorVersion<6 || sameLayout || preservedLayout;
  const compatible = layoutCompatible && valid && saved.version === SAVE_VERSION && saved.campaignSeed === source.seed && (sameGenerator || upgradedGenerator);
  if (compatible) {
    if(hardcoreSource && object(saved.hardcore)){
      const old=saved.hardcore,h=state.hardcore;
      h.currentLevel=validId(old.currentLevel)?Number(old.currentLevel):1;
      h.completedCount=Math.min(h.currentLevel,safeCount(old.completedCount));
      h.bonusCount=Math.min(h.currentLevel*3,safeCount(old.bonusCount));
      h.legacyCompletedCount=old.rewardRevision>=1?Math.min(h.completedCount,safeCount(old.legacyCompletedCount)):h.completedCount;
      h.legacyBonusCount=old.rewardRevision>=1?Math.min(h.bonusCount,safeCount(old.legacyBonusCount)):h.bonusCount;
      h.wordCount=old.rewardRevision>=1?Math.min(capped(h.currentLevel*source.bonusSlotLimit),safeCount(old.wordCount)):0;
      const meta=hardcoreSource.getMeta(h.currentLevel);
      let progress=old.progress;
      if(old.datasetRevision!==source.datasetRevision||old.rulesRevision!==hardcoreSource.rulesRevision){
        const solved=new Set(Array.isArray(progress?.solved)?progress.solved:[]),revealed=new Set();
        for(const word of meta.words)if(solved.has(word.word))word.keys.forEach(key=>revealed.add(key));
        progress={...progress,revealed:[...revealed]};
      }
      h.progress=deriveLevel(meta,progress,old.progress?.rewarded===true&&h.completedCount>0);
      // Old solved words keep their paid status; upgrades never grant rewards again.
      h.rewardedWords=unique(old.rewardRevision>=1?old.rewardedWords:h.progress.solved).filter(word=>typeof word==='string'&&normalizeWord(word)===word).slice(0,source.bonusSlotLimit);
      h.rewardRevision=2;h.legacyEarnings=old.rewardRevision===2?safeCount(old.legacyEarnings):legacyHardcoreEarnings(h);h.rewardCoins=old.rewardRevision===2?Math.min(safeCount(old.rewardCoins),capped(h.currentLevel*source.bonusSlotLimit*18)):0;
      if(saved.mode==='hardcore')state.mode='hardcore';
    }
    if(saved.rackRepairVersion===1)state.rackRepairVersion=1;
    state.dailyGift = sanitizeGift(saved.dailyGift);
    state.completedRanges = normalizeRanges(saved.completedRanges, source.hasLevel);
    state.maxUnlocked = source.reachable(state.completedRanges);
    if(saved.backgrounds?.version===1){
      state.backgrounds.owned=BACKGROUNDS.filter(bg=>bg.price===0||unique(saved.backgrounds.owned).includes(bg.id)).map(bg=>bg.id);
      state.backgrounds.paid=BACKGROUNDS.filter(bg=>bg.price>0&&state.backgrounds.owned.includes(bg.id)&&unique(saved.backgrounds.paid).includes(bg.id)).map(bg=>bg.id);
    }else{
      // Keep backgrounds earned before the shop update; future levels grant none.
      state.backgrounds.owned=BACKGROUNDS.filter(bg=>bg.legacyLevel<=state.maxUnlocked).map(bg=>bg.id);
    }
    if(availableBackground(saved.settings?.background,state.backgrounds.owned))state.settings.background=saved.settings.background;
    if (validId(saved.currentLevel) && source.canStart(saved.currentLevel, state.maxUnlocked)) state.currentLevel = Number(saved.currentLevel);
    state.bonusRanges = Array.from({length:Math.max(3,Math.min(source.bonusSlotLimit,Array.isArray(saved.bonusRanges)?saved.bonusRanges.length:3))},(_,index) => normalizeRanges(saved.bonusRanges?.[index], id => source.canStart(id, state.maxUnlocked)));
    state.hintCount = safeCount(saved.hintCount);
    if (object(saved.legacy) && saved.legacy.version === 1 && saved.migration?.fromVersion === 1 &&
      Number.isSafeInteger(saved.economy?.baseCoins) && saved.economy.baseCoins >= 0 &&
      safeCount(saved.economy.completedBaseline) <= count(state.completedRanges)) {
      state.legacy = clone(saved.legacy);
      state.migration = clone(saved.migration);
      state.economy = { baseCoins: saved.economy.baseCoins, completedBaseline: safeCount(saved.economy.completedBaseline) };
    }
    const savedIds = Object.keys(saved.levels).filter(id => validId(id) && source.canStart(id, state.maxUnlocked) && Number(id) !== state.currentLevel)
      .slice(-(RECENT_LEVEL_LIMIT - 1));
    savedIds.push(String(state.currentLevel));
    for (const id of savedIds) {
      const meta = source.getMeta(id);
      state.levels[id] = deriveLevel(meta, saved.levels[id], contains(state.completedRanges, Number(id)));
      state.levels[id].bonus = state.levels[id].bonus.filter(word => contains(state.bonusRanges[meta.bonuses.indexOf(word)], Number(id)));
      state.levels[id].foundOrder = orderedFoundWords(state.levels[id]);
    }
    state.economy.rewardRevision=2;
    state.economy.legacyClassicCompletions=Math.min(count(state.completedRanges)-state.economy.completedBaseline,saved.economy?.rewardRevision===2?safeCount(saved.economy.legacyClassicCompletions):count(state.completedRanges)-state.economy.completedBaseline);
    const earned = capped(state.economy.baseCoins + (count(state.completedRanges) - state.economy.completedBaseline-state.economy.legacyClassicCompletions) * LEVEL_REWARD+state.economy.legacyClassicCompletions*30 + state.bonusRanges.reduce((sum, ranges) => sum + count(ranges), 0) * BONUS_REWARD + state.dailyGift.count * DAILY_GIFT_REWARD + hardcoreEarnings(state.hardcore));
    const backgroundSpend=BACKGROUNDS.filter(bg=>state.backgrounds.paid.includes(bg.id)).reduce((sum,bg)=>sum+bg.price,0);
    const expected = Math.max(0, earned - state.hintCount * HINT_COST - backgroundSpend);
    state.coins = Number.isSafeInteger(saved.coins) && saved.coins >= 0 ? Math.min(saved.coins, expected) : expected;
  }
  if (!state.levels[state.currentLevel]) state.levels[state.currentLevel] = deriveLevel(source.getMeta(state.currentLevel), null, contains(state.completedRanges, state.currentLevel));
  return attachCompatibility(state, source);
}

export function sanitizeGameState(levelSource, lexicon, saved) { return sanitizePrepared(prepareSource(levelSource, lexicon), saved); }

/** Browser-independent rules; a provider generates only levels actually opened. */
export function createGame(levelSource, lexicon, savedState = null, options = {}) {
  const source = prepareSource(levelSource, lexicon);
  const hardcoreSource=options.hardcoreSource?prepareSource(options.hardcoreSource,lexicon):null;
  let state = sanitizePrepared(source, savedState,hardcoreSource);
  const isHardcore=()=>!!hardcoreSource&&state.mode==='hardcore';
  let rack = [...getMeta().level.letters];
  const visits = new Map(Object.keys(state.levels).map(id => [Number(id), true]));
  function getMeta(id) { return id===undefined&&isHardcore()?hardcoreSource.getMeta(state.hardcore.currentLevel):source.getMeta(id??state.currentLevel); }
  function getLevelState(id) {
    if(id===undefined&&isHardcore())return state.hardcore.progress;
    id??=state.currentLevel;
    const meta = getMeta(id);
    if (!meta) return undefined;
    if (!source.canStart(meta.id, state.maxUnlocked)) return emptyLevel();
    if (!state.levels[meta.id]) {
      state.levels[meta.id] = deriveLevel(meta, null, contains(state.completedRanges, meta.id));
      state.levels[meta.id].bonus = meta.bonuses.filter((word, index) => word&&contains(state.bonusRanges[index], meta.id));
      state.levels[meta.id].foundOrder = orderedFoundWords(state.levels[meta.id]);
    }
    visits.delete(meta.id); visits.set(meta.id, true);
    while (visits.size > RECENT_LEVEL_LIMIT) {
      const old = visits.keys().next().value;
      visits.delete(old);
      if (old === state.currentLevel) visits.set(old, true);
      else delete state.levels[old];
    }
    return state.levels[meta.id];
  }
  function emit() { options.onChange?.(clone(state)); }
  function sync(meta = getMeta(), submittedWord) {
    const progress = getLevelState();
    const before = new Set(progress.solved);
    const previousOrder = orderedFoundWords(progress);
    const shown = new Set(progress.revealed);
    progress.solved = meta.words.filter(item => item.keys.every(key => shown.has(key))).map(item => item.word);
    const newlySolved = progress.solved.filter(word => !before.has(word));
    // Crossings can finish in the same move; keep the entered word last.
    progress.foundOrder = [...previousOrder, ...newlySolved.filter(word => word !== submittedWord),
      ...(newlySolved.includes(submittedWord) ? [submittedWord] : [])];
    progress.completed = progress.solved.length === meta.words.length;
    let completionReward = 0;
    if(isHardcore()){
      const h=state.hardcore,paid=new Set(h.rewardedWords),unpaid=newlySolved.filter(word=>!paid.has(word));
      const reward=unpaid.length*HARDCORE_WORD_REWARD;h.wordCount=capped(h.wordCount+unpaid.length);h.rewardedWords.push(...unpaid);state.coins=capped(state.coins+reward);
      if(progress.completed&&!progress.rewarded){h.completedCount++;completionReward=hardcoreCompletionReward(meta.words.length,progress.bonus.length);h.rewardCoins=capped(h.rewardCoins+completionReward);state.coins=capped(state.coins+completionReward);progress.rewarded=true;}
      return {newlySolved,completed:progress.completed,completionReward,reward};
    }
    if (progress.completed && !contains(state.completedRanges, meta.id)) {
      insertRange(state.completedRanges, meta.id);
      state.coins = capped(state.coins + LEVEL_REWARD);
      completionReward = LEVEL_REWARD;
    }
    progress.rewarded = contains(state.completedRanges, meta.id);
    state.maxUnlocked = source.reachable(state.completedRanges);
    return { newlySolved, completed: progress.completed, completionReward };
  }
  function startLevel(id) {
    if (!validId(id) || !source.canStart(id, state.maxUnlocked)) return false;
    if(hardcoreSource)state.mode='classic';
    const meta = getMeta(id);
    state.currentLevel = meta.id;
    getLevelState();
    rack = [...meta.level.letters];
    emit();
    return true;
  }
  function submit(value) {
    const word = normalizeWord(value);
    const meta = getMeta();
    const progress = getLevelState();
    const length = [...word].length;
    if (meta.excludedLengths.includes(length) && canBuildWord(word, meta.level.letters)) {
      return { type: 'excluded-length', word, length, reward: 0 };
    }
    if (!lexicon.hasWord(word)) return { type: 'invalid', word, reward: 0 };
    if (!lexicon.canBuild(word, meta.level.letters)) return { type: 'unbuildable', word, reward: 0 };
    if (progress.solved.includes(word) || progress.bonus.includes(word)) return { type: 'repeat', word, bonus:progress.bonus.includes(word), reward: 0, completed: progress.completed };
    const target = meta.words.find(item => item.word === word);
    if (target) {
      progress.revealed = unique([...progress.revealed, ...target.keys]);
      const result = { type: 'target', word, reward: 0, ...sync(meta, word) };
      emit(); return result;
    }
    const bonusIndex = meta.bonuses.indexOf(word);
    if (bonusIndex < 0) return { type: 'outside', word, reward: 0 };
    progress.bonus.push(word);
    progress.foundOrder.push(word);
    if(isHardcore())state.hardcore.bonusCount++;
    const reward = isHardcore()?HARDCORE_WORD_REWARD:contains(state.bonusRanges[bonusIndex], meta.id) ? 0 : BONUS_REWARD;
    if (reward) { if(!isHardcore()){while(state.bonusRanges.length<=bonusIndex)state.bonusRanges.push([]);insertRange(state.bonusRanges[bonusIndex], meta.id);} state.coins = capped(state.coins + reward); }
    emit();
    return { type: 'bonus', word, reward, completed: progress.completed, newlySolved: [], completionReward: 0 };
  }
  function getHiddenCells() {
    const revealed = new Set(getLevelState().revealed);
    return getMeta().board.filter(cell => !revealed.has(cell.key));
  }
  function hint() {
    if(isHardcore())return {type:'unavailable',cost:0};
    const hidden = getHiddenCells();
    if (!hidden.length) return { type: 'complete', cost: HINT_COST };
    if (state.coins < HINT_COST) return { type: 'insufficient', cost: HINT_COST };
    const sample = Number((options.random || Math.random)());
    const index = Math.max(0, Math.min(hidden.length - 1, Math.floor((Number.isFinite(sample) ? sample : 0) * hidden.length)));
    const cell = hidden[index];
    state.coins -= HINT_COST;
    state.hintCount = capped(state.hintCount + 1);
    getLevelState().revealed.push(cell.key);
    const result = { type: 'hint', key: cell.key, cost: HINT_COST, ...sync() };
    emit(); return result;
  }
  function shuffle(random = options.random || Math.random) {
    const before = [...rack];
    for (let index = rack.length - 1; index > 0; index--) {
      const sample = Number(random());
      const other = Math.max(0, Math.min(index, Math.floor((Number.isFinite(sample) ? sample : 0) * (index + 1))));
      [rack[index], rack[other]] = [rack[other], rack[index]];
    }
    if (rack.every((letter, index) => letter === before[index])) {
      const other = rack.findIndex(letter => letter !== rack[0]);
      if (other >= 0) [rack[0], rack[other]] = [rack[other], rack[0]];
    }
    return [...rack];
  }
  function setSettings(update = {}) {
    for (const key of ['sound', 'music', 'animations', 'backgroundMotion']) if (typeof update[key] === 'boolean') state.settings[key] = update[key];
    if(availableBackground(update.background,state.backgrounds.owned))state.settings.background=update.background;
    emit(); return { ...state.settings };
  }
  function getDailyGiftStatus() {
    const now = new Date(options.now ? options.now() : Date.now()).getTime(), day = localDay(now);
    const valid = Number.isFinite(now) && now >= 0 && Number.isSafeInteger(day);
    const last = state.dailyGift.lastClaimDay;
    const available = valid && (last === null || day > last);
    let remainingMs = 0;
    if (valid && !available) {
      const date = new Date(last * 86400000);
      const midnight = new Date(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate() + 1).getTime();
      remainingMs = Math.max(0, midnight - now);
    }
    return { available, reward: DAILY_GIFT_REWARD, remainingMs, day };
  }
  function claimDailyGift() {
    const status = getDailyGiftStatus();
    if (!status.available) return { type: 'gift-unavailable', reward: 0 };
    state.dailyGift = { count: state.dailyGift.count + 1, lastClaimDay: status.day };
    state.coins = capped(state.coins + DAILY_GIFT_REWARD);
    emit();
    return { type: 'gift', reward: DAILY_GIFT_REWARD };
  }
  function reset() {
    const settings = { ...state.settings };
    state = sanitizePrepared(source, null,hardcoreSource); state.settings = settings;if(!availableBackground(settings.background,state.backgrounds.owned))delete state.settings.background; visits.clear();
    visits.set(state.currentLevel, true); rack = [...getMeta().level.letters]; emit();
  }
  function replay(id = state.currentLevel) {
    if (!startLevel(id)) return false;
    state.levels[state.currentLevel] = { ...emptyLevel(), rewarded: contains(state.completedRanges, state.currentLevel) };
    emit(); return true;
  }
  function setMode(mode){
    if(!['classic','hardcore'].includes(mode)||(mode==='hardcore'&&!hardcoreSource))return false;
    state.mode=mode;rack=[...getMeta().level.letters];emit();return true;
  }
  function regenerateHardcore(){
    if(!isHardcore()||state.hardcore.currentLevel>=MAX_LEVEL_ID)return false;
    const previous=[...getMeta().level.letters].sort().join('');let attempts=0;
    do{state.hardcore.currentLevel++;attempts++;}while(state.hardcore.currentLevel<MAX_LEVEL_ID&&attempts<20&&[...getMeta().level.letters].sort().join('')===previous);
    state.hardcore.progress=emptyLevel();state.hardcore.rewardedWords=[];rack=[...getMeta().level.letters];emit();return true;
  }
  return {
    ownsBackground:id=>!!availableBackground(id,state.backgrounds.owned),
    purchaseBackground(id){
      const bg=BACKGROUNDS.find(item=>item.id===id);
      if(!bg)return {type:'invalid'};
      if(state.backgrounds.owned.includes(id))return {type:'owned',background:id,cost:0};
      if(state.coins<bg.price)return {type:'insufficient',cost:bg.price};
      state.coins-=bg.price;state.backgrounds.owned.push(id);state.backgrounds.paid.push(id);state.settings.background=id;emit();
      return {type:'purchased',background:id,cost:bg.price};
    },
    get state() { return state; }, get rack() { return [...rack]; }, get completedCount() { return count(state.completedRanges); },get isHardcore(){return isHardcore();},
    getCurrentLevel: () => getMeta().level, getLevel: id => getMeta(id)?.level,
    getMaxUnlocked: () => state.maxUnlocked, getNextLevelId: () => isHardcore()?state.hardcore.currentLevel+1:source.nextId(state.currentLevel),setMode,regenerateHardcore,
    getLevelState, startLevel, submit, hint, shuffle, setSettings, reset, replay, getHiddenCells, getDailyGiftStatus, claimDailyGift,
    getBoard: id => getMeta(id)?.board || [],
    getSolvedWords: id => [...(getLevelState(id)?.solved || [])],
    getFoundWords: id => orderedFoundWords(getLevelState(id)),
    snapshot: () => clone(state),
  };
}



