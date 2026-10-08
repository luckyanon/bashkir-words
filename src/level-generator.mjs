import { createDictionary, normalizeWord, signature } from './lib/dictionary.mjs';
import { getWordCells } from './levels.mjs';
import {createHardcoreLayout} from './hardcore-layout.mjs';

export const GENERATOR_VERSION = 6;
const MAX_SPAN = 24;

function randomSource(value) {
  let state = 2166136261;
  for (const character of String(value)) state = Math.imul(state ^ character.codePointAt(0), 16777619) >>> 0;
  return () => {
    state += 0x6D2B79F5;
    let next = state;
    next = Math.imul(next ^ next >>> 15, next | 1);
    next ^= next + Math.imul(next ^ next >>> 7, next | 61);
    return ((next ^ next >>> 14) >>> 0) / 4294967296;
  };
}

function shuffle(items, random) {
  const result = [...items];
  for (let index = result.length - 1; index > 0; index--) {
    const other = Math.floor(random() * (index + 1));
    [result[index], result[other]] = [result[other], result[index]];
  }
  return result;
}

function boardFor(placements) {
  const board = new Map();
  for (const placement of placements) {
    for (const cell of getWordCells(placement)) {
      const previous = board.get(cell.key);
      if (previous) previous.directions.add(placement.direction);
      else board.set(cell.key, { ...cell, directions: new Set([placement.direction]) });
    }
  }
  return board;
}

function bounds(cells) {
  const rows = cells.map(cell => cell.row);
  const cols = cells.map(cell => cell.col);
  const minRow = Math.min(...rows);
  const minCol = Math.min(...cols);
  return { minRow, minCol, rows: Math.max(...rows) - minRow + 1, cols: Math.max(...cols) - minCol + 1 };
}

function crossingFits(placement, board) {
  const cells = getWordCells(placement);
  const dr = placement.direction === 'down' ? 1 : 0;
  const dc = placement.direction === 'across' ? 1 : 0;
  const first = cells[0];
  const last = cells.at(-1);
  if (board.has(`${first.row - dr},${first.col - dc}`) || board.has(`${last.row + dr},${last.col + dc}`)) return false;
  let intersections = 0;
  for (const cell of cells) {
    const previous = board.get(cell.key);
    if (previous) {
      if (previous.letter !== cell.letter || previous.directions.has(placement.direction)) return false;
      intersections++;
    } else if (board.has(`${cell.row - dc},${cell.col - dr}`) || board.has(`${cell.row + dc},${cell.col + dr}`)) return false;
  }
  const box = bounds([...board.values(), ...cells]);
  return intersections === 1 && box.rows <= 10 && box.cols <= 10;
}

/** Small, independently positioned clusters prevent the same dense rectangle on every level. */
function createClusters(source, random) {
  const pending = shuffle(source, random);
  const clusters = [];
  while (pending.length) {
    const first = pending.shift();
    const placements = [{ ...first, row: 0, col: 0, direction: random() < 0.5 ? 'across' : 'down' }];
    const limit = 2 + Math.floor(random() * 3);
    while (pending.length && placements.length < limit) {
      const board = boardFor(placements);
      const choices = [];
      for (const [pendingIndex, entry] of pending.entries()) {
        const seen = new Set();
        for (const cell of board.values()) {
          for (const [offset, letter] of [...entry.word].entries()) {
            if (letter !== cell.letter) continue;
            for (const direction of ['across', 'down']) {
              if (cell.directions.has(direction)) continue;
              const row = cell.row - (direction === 'down' ? offset : 0);
              const col = cell.col - (direction === 'across' ? offset : 0);
              const key = `${row},${col},${direction}`;
              if (seen.has(key)) continue;
              seen.add(key);
              const placement = { ...entry, row, col, direction };
              if (crossingFits(placement, board)) choices.push({ placement, pendingIndex });
            }
          }
        }
      }
      if (!choices.length) break;
      const choice = choices[Math.floor(random() * choices.length)];
      placements.push(choice.placement);
      pending.splice(choice.pendingIndex, 1);
    }
    const box = bounds([...boardFor(placements).values()]);
    clusters.push({
      rows: box.rows, cols: box.cols,
      words: placements.map(placement => ({ ...placement, row: placement.row - box.minRow, col: placement.col - box.minCol })),
    });
  }
  return shuffle(clusters, random);
}

function detachedFits(cells, board) {
  for (const cell of cells) {
    if (board.has(cell.key)) return false;
    for (const [dr, dc] of [[-1, 0], [1, 0], [0, -1], [0, 1]]) {
      if (board.has(`${cell.row + dr},${cell.col + dc}`)) return false;
    }
  }
  return true;
}

function packClusters(clusters, random) {
  let placements = clusters[0].words;
  for (const cluster of clusters.slice(1)) {
    const board = boardFor(placements);
    const current = bounds([...board.values()]);
    const cells = [...boardFor(cluster.words).values()];
    const candidates = [];
    // Every direction is considered, including a separate strip above or beside the field.
    const preferredEdge = Math.floor(random() * 4);
    for (let row = current.minRow - cluster.rows - 2; row <= current.minRow + current.rows + 1; row++) {
      for (let col = current.minCol - cluster.cols - 2; col <= current.minCol + current.cols + 1; col++) {
        const translated = cells.map(cell => ({ ...cell, row: cell.row + row, col: cell.col + col, key: `${cell.row + row},${cell.col + col}` }));
        if (!detachedFits(translated, board)) continue;
        const box = bounds([...board.values(), ...translated]);
        if (box.rows > MAX_SPAN || box.cols > MAX_SPAN) continue;
        const edge = row + cluster.rows <= current.minRow ? 0
          : col >= current.minCol + current.cols ? 1
            : row >= current.minRow + current.rows ? 2 : 3;
        const area = box.rows * box.cols;
        const aspectPenalty = Math.abs(box.rows - box.cols) * 3;
        // A random edge preference changes the outline; compactness keeps cells readable.
        const longestSide = Math.max(box.rows, box.cols);
        const score = area + longestSide * longestSide * 0.65 + aspectPenalty
          + (edge === preferredEdge ? -18 : 0) + random() * 22;
        candidates.push({ row, col, score });
      }
    }
    candidates.sort((a, b) => a.score - b.score);
    if (!candidates.length) throw new Error('Cannot place every sourced word within the free field');
    const choice = candidates[0];
    placements = placements.concat(cluster.words.map(word => ({ ...word, row: word.row + choice.row, col: word.col + choice.col })));
  }
  const box = bounds([...boardFor(placements).values()]);
  return {
    rows: box.rows, cols: box.cols,
    words: placements.map(placement => ({ ...placement, row: placement.row - box.minRow, col: placement.col - box.minCol })),
  };
}

function compactCrossing(placement, board) {
  const cells = getWordCells(placement);
  const dr = placement.direction === 'down' ? 1 : 0;
  const dc = placement.direction === 'across' ? 1 : 0;
  const first = cells[0];
  const last = cells.at(-1);
  if (board.has(`${first.row - dr},${first.col - dc}`) || board.has(`${last.row + dr},${last.col + dc}`)) return null;
  let intersections = 0;
  for (const cell of cells) {
    const previous = board.get(cell.key);
    if (previous) {
      if (previous.letter !== cell.letter || previous.directions.has(placement.direction)) return null;
      intersections++;
    } else if (board.has(`${cell.row - dc},${cell.col - dr}`) || board.has(`${cell.row + dc},${cell.col + dr}`)) return null;
  }
  const box = bounds([...board.values(), ...cells]);
  if (!intersections || box.rows > MAX_SPAN || box.cols > MAX_SPAN) return null;
  return { box, intersections };
}

/** Grow each connected field until no remaining word fits; detached groups are a last resort. */
function createCompactClusters(source, random) {
  const pending = shuffle(source, random).sort((a, b) => [...b.word].length - [...a.word].length);
  const clusters = [];
  while (pending.length) {
    const first = pending.shift();
    const placements = [{ ...first, row: 0, col: 0, direction: random() < 0.5 ? 'across' : 'down' }];
    while (pending.length) {
      const board = boardFor(placements);
      const choices = [];
      for (const [pendingIndex, entry] of pending.entries()) {
        const seen = new Set();
        for (const cell of board.values()) {
          for (const [offset, letter] of [...entry.word].entries()) {
            if (letter !== cell.letter) continue;
            for (const direction of ['across', 'down']) {
              if (cell.directions.has(direction)) continue;
              const row = cell.row - (direction === 'down' ? offset : 0);
              const col = cell.col - (direction === 'across' ? offset : 0);
              const key = `${row},${col},${direction}`;
              if (seen.has(key)) continue;
              seen.add(key);
              const placement = { ...entry, row, col, direction };
              const crossing = compactCrossing(placement, board);
              if (!crossing) continue;
              const { box, intersections } = crossing;
              const score = box.rows * box.cols + Math.max(box.rows, box.cols) ** 2 * 0.3
                - intersections * 25 + random() * 3;
              choices.push({ placement, pendingIndex, score });
            }
          }
        }
      }
      if (!choices.length) break;
      choices.sort((a, b) => a.score - b.score);
      const choice = choices[0];
      placements.push(choice.placement);
      pending.splice(choice.pendingIndex, 1);
    }
    const box = bounds([...boardFor(placements).values()]);
    clusters.push({
      rows: box.rows, cols: box.cols,
      words: placements.map(placement => ({ ...placement, row: placement.row - box.minRow, col: placement.col - box.minCol })),
    });
  }
  return shuffle(clusters, random);
}

function createCompactLayout(source, seed, fallback, fallbackComponents) {
  // Connectivity takes priority; every area/shape penalty fits below one component's weight.
  const scoreFor = (layout, components) => components * 10000 + layout.rows * layout.cols
    + Math.max(layout.rows, layout.cols) ** 2 * 0.3;
  let best = fallback;
  let bestScore = scoreFor(fallback, fallbackComponents);
  for (let attempt = 0; attempt < 8; attempt++) {
    const random = randomSource(`${seed}:${attempt}`);
    const clusters = createCompactClusters(source, random);
    let candidate;
    try {
      candidate = packClusters(clusters, random);
    } catch {
      // Another starting direction/order can fit when this greedy candidate cannot be packed.
      continue;
    }
    const score = scoreFor(candidate, clusters.length);
    if (score < bestScore) {
      best = candidate;
      bestScore = score;
    }
  }
  return best;
}

const CATEGORY_DECK = Object.freeze([...Array(8).fill(5), ...Array(8).fill(6), ...Array(3).fill(7), 8]);
const CATEGORY_COUNTS = Object.freeze({ 5: 8, 6: 8, 7: 3, 8: 1 });
const validLevelId = id => Number.isSafeInteger(id) && id >= 1;
export const minWordLengthForRack = rackLength => rackLength === 5 ? 2 : rackLength === 6 ? 3 : 4;

/** A seeded shuffled deck gives the requested ratio and guarantees every category in twenty levels. */
export function getRackCategory(id, seed = 'bashkir-words') {
  if (!validLevelId(id)) throw new RangeError('Level number must be a positive safe integer');
  const block = Math.floor((id - 1) / CATEGORY_DECK.length);
  const offset = (id - 1) % CATEGORY_DECK.length;
  return shuffle(CATEGORY_DECK, randomSource(`${GENERATOR_VERSION}:${String(seed)}:sizes:${block}`))[offset];
}

import {maxWordLengthForLevel} from './difficulty.mjs';
export {maxWordLengthForLevel} from './difficulty.mjs';

/** Only sourced nouns, with a deterministic probability deck and stage limits. */
export function createLevelProvider(poolData, lexicon, {
  seed = 'bashkir-words', generatorVersion = poolData?.generatorVersion,
  layoutVersion = 2, legacyLayoutThroughLevel = 0, includeLocalWords = true, applyLevelDifficulty = true, hardcore = false,
} = {}) {
  if (![1, 2].includes(layoutVersion)) throw new RangeError('Unsupported layout version');
  if (!Number.isSafeInteger(legacyLayoutThroughLevel) || legacyLayoutThroughLevel < 0) throw new RangeError('Legacy layout boundary must be a nonnegative safe integer');
  if(generatorVersion!==6 || poolData?.schemaVersion!=='6.0.0' || poolData.partOfSpeech!=='noun' || !Number.isSafeInteger(poolData.maxWordLength) || poolData.maxWordLength<9 || poolData.maxWordLength>12 || !poolData.pools?.length)throw new TypeError('Expected noun level pools');
  const dictionary = lexicon?.dictionary || (typeof lexicon?.hasWord === 'function' ? lexicon : createDictionary(lexicon));
  const pools = poolData.pools;
  const groups = new Map([5, 6, 7, 8].map(category => [category, []]));
  for (const [index, pool] of pools.entries()) {
    if (!Array.isArray(pool.letters) || pool.letters.length < 5 || pool.letters.length > poolData.maxWordLength
      || !Array.isArray(pool.words) || pool.words.length < 7 || pool.words.length > 12
      || !Array.isArray(pool.bonusWords) || pool.bonusWords.length > 3
      || pool.minWordLength !== minWordLengthForRack(pool.letters.length)
      || !pool.words.some(entry => [...entry.word].length === pool.letters.length && signature(entry.word) === pool.letters.join(''))) throw new TypeError('Invalid level pool limits or missing full-rack target');
    const excludedWordLengths = Array.from({ length: pool.minWordLength - 2 }, (_, index) => index + 2);
    if (!Array.isArray(pool.excludedWordLengths) || JSON.stringify(pool.excludedWordLengths) !== JSON.stringify(excludedWordLengths)) {
      throw new TypeError('Invalid excluded word lengths');
    }
    const declared = [...pool.words.map(entry => entry.word), ...pool.bonusWords];
    if (new Set(declared).size !== declared.length || signature(pool.letters.join('')) !== pool.letters.join('')
      || declared.some(word => normalizeWord(word) !== word || [...word].length < pool.minWordLength || !dictionary.canBuild(word, pool.letters))
      || pool.words.some(entry => !dictionary.isTargetCandidate(entry.word) || typeof entry.entryId !== 'string' || !entry.entryId)) {
      throw new TypeError('Level pool contains invalid or unsourced words');
    }
    if (typeof lexicon?.getEntry === 'function') {
      for (const target of pool.words) {
        const entry = lexicon.getEntry(target.word);
        if (!entry || !entry.partsOfSpeech?.includes('noun') || entry.id !== target.entryId || normalizeWord(entry.display || entry.word) !== target.word
          || !entry.definitions?.ru?.some(meaning => typeof meaning?.text === 'string' && meaning.text.trim())) {
          throw new TypeError('Level pool target must retain its original entry ID and Russian meaning');
        }
      }
    }
    const category = Math.min(8, pool.letters.length);
    groups.get(category).push(index);
  }
  if ([...groups.values()].some(indexes => !indexes.length)) throw new TypeError('Every weighted rack category must have a source pool');
  const campaignSeed = String(seed);
  const cache = new Map();
  const hardcoreIndexes=hardcore?pools.map((pool,index)=>({pool,index})).filter(({pool})=>Number.isSafeInteger(pool.hardcoreWordCount)&&pool.hardcoreWordCount>=30).map(({index})=>index):[];
  if(hardcore&&!hardcoreIndexes.length)throw new TypeError('Hardcore requires eligible racks with at least 30 targets');
  const hardcoreOrders=new Map();
  const hasLevel = validLevelId;
  function getLevel(id) {
    if (!hasLevel(id)) throw new RangeError('Level number must be a positive safe integer');
    if (cache.has(id)) return cache.get(id);
    if(hardcore){
      const cap=maxWordLengthForLevel(id,poolData.maxWordLength);
      const indexes=hardcoreIndexes.filter(index=>pools[index].letters.length<=cap);
      if(!indexes.length)throw new TypeError('No eligible hardcore racks within the length limit');
      const cycle=Math.floor((id-1)/indexes.length),key=`${cap}:${cycle}`;
      if(!hardcoreOrders.has(key)){hardcoreOrders.clear();hardcoreOrders.set(key,shuffle(indexes,randomSource(`${campaignSeed}:hardcore:2:pools:${key}`)));}
      const pool=pools[hardcoreOrders.get(key)[(id-1)%indexes.length]];
      const words=lexicon.findWords(pool.letters,{minLength:2,targetsOnly:true});
      if(words.length<30||words.length!==pool.hardcoreWordCount)throw new TypeError('Outdated hardcore rack index');
      const layout=createHardcoreLayout(words.map(word=>({word,entryId:lexicon.getEntry(word).id})),randomSource(`${campaignSeed}:hardcore:2:${id}`));
      const bonusWords=lexicon.findWords(pool.letters,{minLength:2}).filter(word=>!lexicon.isTargetCandidate(word)).slice(0,3);
      const level=Object.freeze({id,title:'Хардкор',mode:'hardcore',letters:Object.freeze(shuffle(pool.letters,randomSource(`${campaignSeed}:hardcore:rack:${id}`))),words:Object.freeze(layout.words.map(Object.freeze)),bonusWords:Object.freeze(bonusWords),rows:layout.rows,cols:layout.cols,layout:'free',difficultyRevision:applyLevelDifficulty?1:0,generatorVersion:GENERATOR_VERSION,layoutVersion:2,minWordLength:2,excludedWordLengths:Object.freeze([])});
      cache.set(id,level);if(cache.size>8)cache.delete(cache.keys().next().value);return level;
    }
    // Start a fresh probability deck at the first new level after a saved v2 boundary.
    const newId = id;
    const block = Math.floor((newId - 1) / CATEGORY_DECK.length);
    const offset = (newId - 1) % CATEGORY_DECK.length;
    const deck = shuffle(CATEGORY_DECK, randomSource(`${GENERATOR_VERSION}:${campaignSeed}:sizes:${block}`));
    const minimum=applyLevelDifficulty?(id>80?4:id>40?3:2):2;
    const category = minimum===4 && deck[offset]===5 ? 6 : deck[offset];
    const ordinal = block * CATEGORY_COUNTS[category] + deck.slice(0, offset).filter(value => value === category).length;
    const indexes = groups.get(category).filter(index=>pools[index].letters.length<=maxWordLengthForLevel(id,poolData.maxWordLength) && pools[index].words.filter(entry=>entry.word.length>=minimum).length>=7);
    const cycle = Math.floor(ordinal / indexes.length);
    const order = shuffle(indexes, randomSource(`${GENERATOR_VERSION}:${campaignSeed}:pools:${category}:${cycle}`));
    const originalPool = pools[order[ordinal % indexes.length]];
    const minWordLength=Math.max(minimum,originalPool.minWordLength);
    const pool={...originalPool,minWordLength,words:originalPool.words.filter(entry=>entry.word.length>=minWordLength),bonusWords:originalPool.bonusWords.filter(word=>word.length>=minWordLength),excludedWordLengths:Array.from({length:minWordLength-2},(_,i)=>i+2)};
    const random = randomSource(`${GENERATOR_VERSION}:${campaignSeed}:${id}`);
    // Keep the original random stream for rack order, independently of how cells are arranged.
    const clusters = createClusters(pool.words, random);
    const legacyLayout = packClusters(clusters, random);
    const letters = shuffle(pool.letters, random);
    const currentLayoutVersion = layoutVersion === 1 || id <= legacyLayoutThroughLevel ? 1 : 2;
    let layout = currentLayoutVersion === 1 ? legacyLayout
      : createCompactLayout(pool.words, `${GENERATOR_VERSION}:${campaignSeed}:${id}:layout:2`, legacyLayout, clusters.length);
    const additions=includeLocalWords?(lexicon.additionalWords||[]).filter(word=>word.length>=pool.minWordLength&&dictionary.canBuild(word,pool.letters)&&!pool.words.some(w=>w.word===word)):[];
    if(additions.length){const words=[...pool.words,...additions.map(word=>({word,entryId:lexicon.getEntry(word).id}))];layout=createCompactLayout(words,`${GENERATOR_VERSION}:${campaignSeed}:${id}:local-words:1`,packClusters(createCompactClusters(words,randomSource(`${campaignSeed}:${id}:local-fallback`)),randomSource(`${campaignSeed}:${id}:local-pack`)),words.length);}
    const level = Object.freeze({
      id, title: 'Башкирские слова', letters: Object.freeze(letters),
      words: Object.freeze(layout.words.map(word => Object.freeze(word))),
      bonusWords: Object.freeze([...pool.bonusWords]), rows: layout.rows, cols: layout.cols,
      layout: 'free', difficultyRevision:applyLevelDifficulty?1:0, generatorVersion: GENERATOR_VERSION, layoutVersion: currentLayoutVersion, minWordLength: pool.minWordLength,
      excludedWordLengths: Object.freeze([...pool.excludedWordLengths]),
    });
    cache.set(id, level);
    if (cache.size > 8) cache.delete(cache.keys().next().value);
    return level;
  }
  return Object.freeze({ getLevel, hasLevel, size: Infinity, poolCount: pools.length,
    mode:hardcore?'hardcore':'classic',rulesRevision:hardcore?2:0,seed: campaignSeed, datasetRevision:poolData.datasetRevision, generatorVersion: GENERATOR_VERSION, layoutVersion, legacyLayoutThroughLevel,wordRevision:includeLocalWords?1:0,difficultyRevision:applyLevelDifficulty?1:0,
    legacyThroughLevel:0, legacyV3ThroughLevel:0, maxWordLength:poolData.maxWordLength,
    probabilityDeckSize: CATEGORY_DECK.length });
}



