import {maxWordLengthForLevel} from './difficulty.mjs';
import { isAlphabetWord, normalizeWord } from './lib/dictionary.mjs';

export const LEVEL_LIMITS = Object.freeze({ minRack: 3, maxRack: 12, maxRows: 12, maxCols: 12, maxWords: 13, freeMaxRows: 24, freeMaxCols: 24 });

/** A placement is stored once; intersections are derived from its coordinates. */
export function getWordCells(placement) {
  if (!placement || typeof placement.word !== 'string' || !Number.isInteger(placement.row)
    || !Number.isInteger(placement.col) || !['across', 'down'].includes(placement.direction)) {
    throw new TypeError('Invalid word placement');
  }
  return [...placement.word].map((letter, offset) => {
    const row = placement.row + (placement.direction === 'down' ? offset : 0);
    const col = placement.col + (placement.direction === 'across' ? offset : 0);
    return { row, col, key: `${row},${col}`, letter };
  });
}

/** Each shared cell lists every word which it can reveal or translate. */
export function buildBoard(level) {
  const cells = new Map();
  for (const [wordIndex, placement] of level.words.entries()) {
    for (const cell of getWordCells(placement)) {
      const previous = cells.get(cell.key);
      if (previous && previous.letter !== cell.letter) throw new Error(`Conflicting letters at ${cell.key}`);
      if (previous) previous.wordIndexes.push(wordIndex);
      else cells.set(cell.key, { ...cell, wordIndexes: [wordIndex] });
    }
  }
  return [...cells.values()].sort((a, b) => a.row - b.row || a.col - b.col);
}

function resolveEntry(service, word, entryId) {
  if (typeof service?.getEntry === 'function') return service.getEntry(word);
  if (service?.entries instanceof Map) return service.entries.get(word);
  if (Array.isArray(service?.entries)) {
    return service.entries.find(entry => normalizeWord(entry.display || entry.word) === word);
  }
  if (typeof service?.getEntryById === 'function') return service.getEntryById(entryId);
  return undefined;
}

/** Does not throw: malformed external level data produces actionable validation errors. */
export function validateLevel(level, service) {
  const errors = [];
  const fail = text => errors.push(text);
  const dictionary = service?.dictionary || service;
  const result = (cells = [], stats = {}) => ({ valid: errors.length === 0, errors, cells, stats });
  if (!level || typeof level !== 'object' || Array.isArray(level)) {
    fail('Уровень должен быть объектом.');
    return result();
  }
  if (!Number.isSafeInteger(level.id) || level.id < 1) fail('Некорректный номер уровня.');
  if (typeof level.title !== 'string' || !level.title.trim()) fail('У уровня нет названия.');
  const freeLayout = level.layout === 'free';
  const hardcore = level.mode === 'hardcore' && level.generatorVersion === 6;
  const generated = Number(level.generatorVersion) >= 2;
  const filtered = Number(level.generatorVersion) >= 3;
  const expectedMinWordLength = hardcore?2:Math.max(level.letters?.length === 5 ? 2 : level.letters?.length === 6 ? 3 : 4,level.difficultyRevision ? (level.id>80?4:level.id>40?3:2) : 2);
  const maxRows = hardcore ? 2048 : freeLayout ? LEVEL_LIMITS.freeMaxRows : LEVEL_LIMITS.maxRows;
  const maxCols = hardcore ? 2048 : freeLayout ? LEVEL_LIMITS.freeMaxCols : LEVEL_LIMITS.maxCols;
  if (!Number.isInteger(level.rows) || level.rows < 1 || level.rows > maxRows
    || !Number.isInteger(level.cols) || level.cols < 1 || level.cols > maxCols) {
    fail(`Размеры поля должны быть целыми числами от 1 до ${maxRows}.`);
  }
  const validRack = Array.isArray(level.letters)
    && level.letters.length >= LEVEL_LIMITS.minRack && level.letters.length <= LEVEL_LIMITS.maxRack
    && level.letters.every(letter => typeof letter === 'string' && [...letter].length === 1
      && normalizeWord(letter) === letter && isAlphabetWord(letter));
  if (!validRack) fail('Круг должен содержать от 3 до 12 отдельных башкирских букв.');
  if(Number(level.generatorVersion)>=5){const cap=maxWordLengthForLevel(level.id);if(level.letters?.length>cap)fail('Длина слова превышает предел сложности этого уровня.');}
  if (filtered) {
    if (!validRack || level.letters.length < 5 || level.minWordLength !== expectedMinWordLength) fail('Некорректная минимальная длина слова для уровня.');
    const excluded = Array.from({ length: expectedMinWordLength - 2 }, (_, index) => index + 2);
    if (JSON.stringify(level.excludedWordLengths) !== JSON.stringify(excluded)) fail('Некорректные исключённые длины слов.');
  }
  if (!Array.isArray(level.words) || level.words.length === 0 || (!hardcore && level.words.length > LEVEL_LIMITS.maxWords)) {
    fail('Уровень должен содержать от 1 до 13 целевых слов.');
    return result();
  }
  if (generated && level.words.length < 7) fail('Сгенерированный уровень должен содержать не менее 7 целевых слов.');
  if(hardcore&&level.words.length<30)fail('Хардкор должен содержать не менее 30 целевых слов.');
  if(hardcore && typeof dictionary?.findWords==='function'){
    const expected=dictionary.findWords(level.letters,{minLength:expectedMinWordLength,targetsOnly:true});
    const targets=new Set(level.words.map(word=>word.word));
    if(expected.length!==targets.size||expected.some(word=>!targets.has(word)))fail('Хардкор должен включать все подходящие целевые слова.');
  }
  if (Number(level.generatorVersion) >= 4 && !level.words.some(placement => [...(placement?.word || '')].length === level.letters?.length)) fail('На уровне должно быть целевое слово, использующее все буквы круга.');
  const hasDictionary = ['hasWord', 'isTargetCandidate', 'canBuild']
    .every(method => typeof dictionary?.[method] === 'function');
  if (!hasDictionary) fail('Нет модуля проверки игрового словаря.');

  const placements = [];
  const wordKeys = new Set();
  const declaredChains = new Set();
  const board = new Map();
  const adjacency = level.words.map(() => new Set());
  for (const [index, placement] of level.words.entries()) {
    if (!placement || typeof placement !== 'object') {
      fail(`Слово ${index + 1}: некорректная запись.`);
      continue;
    }
    const word = placement.word;
    if (typeof word !== 'string' || normalizeWord(word) !== word || !isAlphabetWord(word)
      || [...word].length < 2 || [...word].length > (Number(level.generatorVersion) >= 5 ? 12 : Number(level.generatorVersion) >= 4 ? 12 : 8)) {
      fail(`Слово ${index + 1}: нужно нормализованное слово из 2–${Number(level.generatorVersion) >= 5 ? 12 : Number(level.generatorVersion) >= 4 ? 12 : 8} букв.`);
      continue;
    }
    if (wordKeys.has(word)) fail(`Повтор целевого слова: ${word}.`);
    if (filtered && [...word].length < expectedMinWordLength) fail(`${word}: слово короче разрешённой длины.`);
    wordKeys.add(word);
    if (hasDictionary) {
      try {
        if (!dictionary.hasWord(word)) fail(`${word}: нет в игровом словаре.`);
        if (!dictionary.isTargetCandidate(word)) fail(`${word}: не разрешено как целевое слово.`);
        if (validRack && !dictionary.canBuild(word, level.letters)) fail(`${word}: не хватает букв круга.`);
      } catch {
        fail(`${word}: ошибка проверки словаря.`);
      }
    }
    let entry;
    try { entry = resolveEntry(service, word, placement.entryId); }
    catch { fail(`${word}: ошибка доступа к подробной записи.`); }
    if (!entry || normalizeWord(entry.display || entry.word) !== word) fail(`${word}: нет подробной записи.`);
    if (typeof placement.entryId !== 'string' || !placement.entryId || entry?.id !== placement.entryId) {
      fail(`${word}: некорректная связь со стабильным id записи.`);
    }
    if (!Array.isArray(entry?.definitions?.ru)
      || !entry.definitions.ru.some(meaning => typeof meaning?.text === 'string' && meaning.text.trim())) {
      fail(`${word}: нет русского значения.`);
    }
    if (!Number.isInteger(placement.row) || !Number.isInteger(placement.col)
      || !['across', 'down'].includes(placement.direction)) {
      fail(`${word}: некорректные координаты или направление.`);
      continue;
    }
    const cells = getWordCells(placement);
    placements.push({ placement, cells, index });
    declaredChains.add(`${placement.direction}:${cells.map(cell => cell.key).join('|')}:${word}`);
    for (const cell of cells) {
      if (cell.row < 0 || cell.col < 0 || cell.row >= level.rows || cell.col >= level.cols) {
        fail(`${word}: клетка ${cell.key} вне поля.`);
      }
      const previous = board.get(cell.key);
      if (previous) {
        if (previous.letter !== cell.letter) fail(`Разные буквы на пересечении ${cell.key}.`);
        for (const otherIndex of previous.wordIndexes) {
          if (level.words[otherIndex].direction === placement.direction) fail(`Наложение слов в направлении ${placement.direction} на ${cell.key}.`);
          else {
            adjacency[index].add(otherIndex);
            adjacency[otherIndex].add(index);
          }
        }
        previous.wordIndexes.push(index);
      } else board.set(cell.key, { ...cell, wordIndexes: [index] });
    }
  }

  // Every contiguous run of at least two cells must exactly match one placement.
  // This catches touching endpoints, unintended side-by-side chains and tails.
  for (const direction of ['across', 'down']) {
    const dr = direction === 'down' ? 1 : 0;
    const dc = direction === 'across' ? 1 : 0;
    for (const cell of board.values()) {
      if (board.has(`${cell.row - dr},${cell.col - dc}`)) continue;
      const run = [];
      let row = cell.row;
      let col = cell.col;
      while (board.has(`${row},${col}`)) {
        run.push(board.get(`${row},${col}`));
        row += dr;
        col += dc;
      }
      if (run.length >= 2) {
        const chainKey = `${direction}:${run.map(item => item.key).join('|')}:${run.map(item => item.letter).join('')}`;
        if (!declaredChains.has(chainKey)) fail(`Незаданная цепочка клеток ${direction} от ${cell.key}.`);
      }
    }
  }
  if (placements.length !== level.words.length) fail('Не все слова могут быть размещены на поле.');
  const visited = new Set();
  const pending = [0];
  while (pending.length) {
    const index = pending.pop();
    if (visited.has(index)) continue;
    visited.add(index);
    for (const otherIndex of adjacency[index] || []) pending.push(otherIndex);
  }
  if (!freeLayout && visited.size !== level.words.length) fail('Слова должны образовывать связный кроссворд через пересечения.');
  if (generated) {
    if (!Array.isArray(level.bonusWords) || level.bonusWords.length > 3) fail('Уровень должен объявлять не более 3 бонусных слов.');
    else {
      const bonuses = new Set(level.bonusWords);
      if (bonuses.size !== level.bonusWords.length) fail('Бонусные слова не должны повторяться.');
      for (const word of bonuses) {
        if (typeof word !== 'string' || normalizeWord(word) !== word || !isAlphabetWord(word)) fail('Некорректное бонусное слово.');
        else if (wordKeys.has(word)) fail(`${word}: бонусное слово уже размещено на поле.`);
        else if (hasDictionary && !dictionary.canBuild(word, level.letters)) fail(`${word}: бонусное слово нельзя собрать из круга.`);
        else if (filtered && [...word].length < expectedMinWordLength) fail(`${word}: бонусное слово короче разрешённой длины.`);
      }
      if (Number(level.generatorVersion)<5 && validRack && typeof dictionary?.findWords === 'function') {
        const buildable = dictionary.findWords(level.letters, { ...(filtered ? { minLength: expectedMinWordLength } : {}), maxLength: Number(level.generatorVersion) >= 5 ? 12 : Number(level.generatorVersion) >= 4 ? 12 : 8 });
        const declared = new Set([...wordKeys, ...bonuses]);
        if (buildable.some(word => !declared.has(word)) || declared.size !== buildable.length) {
          fail('Все слова, собираемые из букв уровня, должны быть целевыми или бонусными.');
        }
      }
    }
  }
  const cells = [...board.values()].sort((a, b) => a.row - b.row || a.col - b.col);
  return result(cells, {
    wordCount: level.words.length,
    cellCount: cells.length,
    intersectionCount: cells.filter(cell => cell.wordIndexes.length > 1).length,
    componentCount: (() => {
      const unseen = new Set(level.words.map((_, index) => index));
      let count = 0;
      while (unseen.size) {
        count++;
        const queue = [unseen.values().next().value];
        while (queue.length) {
          const index = queue.pop();
          if (!unseen.delete(index)) continue;
          queue.push(...(adjacency[index] || []));
        }
      }
      return count;
    })(),
    rackLength: Array.isArray(level.letters) ? level.letters.length : 0,
  });
}

/** Validates an entire ordered campaign, including progression ids and duplicates. */
export function validateLevels(levels, service) {
  if (!Array.isArray(levels)) return { valid: false, errors: ['Список уровней должен быть массивом.'], reports: [] };
  const errors = [];
  const signatures = new Set();
  const reports = levels.map((level, index) => {
    const report = validateLevel(level, service);
    if (level?.id !== index + 1) errors.push(`Номер уровня ${index + 1} нарушает порядок.`);
    const signature = Array.isArray(level?.words) ? level.words.map(word => word?.word).sort().join('|') : '';
    if (![2, 3].includes(level?.generatorVersion) && signatures.has(signature)) errors.push(`Уровень ${index + 1} повторяет набор целевых слов.`);
    signatures.add(signature);
    errors.push(...report.errors.map(error => `Уровень ${index + 1}: ${error}`));
    return report;
  });
  return { valid: errors.length === 0, errors, reports };
}

