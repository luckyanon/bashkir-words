/** Browser/Android-compatible helper. No network, DOM or Node dependencies. */
export const ALPHABET = 'АБВГҒДҘЕЁЖЗИЙКҠЛМНҢОӨПРСҪТУҮФХҺЦЧШЩЪЫЬЭӘЮЯ';
const allowed = new Set(ALPHABET);
const rank = new Map([...ALPHABET].map((letter, i) => [letter, i]));

/** NFC, removes accent notation and discretionary soft hyphens; letters remain distinct. */
export function normalizeWord(value) {
  if (typeof value !== 'string') return '';
  return value.normalize('NFC').replace(/[\u0301\u0341\u00ad]/g, '').normalize('NFC').trim().toUpperCase();
}

export function isAlphabetWord(value) {
  const word = normalizeWord(value);
  return word.length > 0 && [...word].every(letter => allowed.has(letter));
}

export function letterCounts(value) {
  const word = normalizeWord(Array.isArray(value) ? value.join('') : value);
  if (!isAlphabetWord(word)) throw new TypeError('Expected Bashkir Cyrillic letters only');
  const counts = Object.create(null);
  for (const letter of word) counts[letter] = (counts[letter] || 0) + 1;
  return counts;
}

export function signature(value) {
  if (!isAlphabetWord(value)) throw new TypeError('Expected Bashkir Cyrillic word');
  return [...normalizeWord(value)].sort((a, b) => rank.get(a) - rank.get(b)).join('');
}

/** A tile is consumed once; repeated letters need repeated tiles. */
export function canBuildWord(wordValue, rackValue) {
  const word = normalizeWord(wordValue);
  const rack = normalizeWord(Array.isArray(rackValue) ? rackValue.join('') : rackValue);
  if (!isAlphabetWord(word) || !isAlphabetWord(rack)) return false;
  if (Array.isArray(rackValue) && rackValue.some(tile => normalizeWord(tile).length !== 1)) return false;
  const available = letterCounts(rack);
  for (const letter of word) {
    if (!(available[letter] > 0)) return false;
    available[letter]--;
  }
  return true;
}

export function createDictionary(data) {
  if (!data || data.schemaVersion !== '1.0.0' || data.language !== 'ba' || !Array.isArray(data.words)) {
    throw new TypeError('Unsupported dictionary format');
  }
  const words = new Set(data.words);
  if (words.size !== data.words.length || [...words].some(word => !isAlphabetWord(word) || normalizeWord(word) !== word)) {
    throw new TypeError('Dictionary must contain unique normalized Bashkir words');
  }
  const targets = new Set(data.targetWords || []);
  if ([...targets].some(word => !words.has(word))) throw new TypeError('Target words must belong to the dictionary');
  const minLength = data.minLength || 2;
  const maxLength = data.maxLength || 8;
  const buckets = new Map();
  for (const word of words) {
    const length = [...word].length;
    if (length < minLength || length > maxLength) throw new TypeError('Word outside the declared length range');
    if (!buckets.has(length)) buckets.set(length, []);
    buckets.get(length).push({ word, counts: letterCounts(word) });
  }
  return Object.freeze({
    size: words.size,
    normalize: normalizeWord,
    hasWord: value => words.has(normalizeWord(value)),
    isTargetCandidate: value => targets.has(normalizeWord(value)),
    canBuild: (word, rack) => words.has(normalizeWord(word)) && canBuildWord(word, rack),
    findWords(rackValue, options = {}) {
      const rack = normalizeWord(Array.isArray(rackValue) ? rackValue.join('') : rackValue);
      if (!isAlphabetWord(rack) || (Array.isArray(rackValue) && rackValue.some(tile => normalizeWord(tile).length !== 1))) return [];
      const available = letterCounts(rack);
      const found = [];
      const min = Math.max(minLength, options.minLength ?? minLength);
      const max = Math.min(maxLength, [...rack].length, options.maxLength ?? maxLength);
      for (let length = min; length <= max; length++) {
        for (const item of buckets.get(length) || []) {
          if (options.targetsOnly && !targets.has(item.word)) continue;
          if (Object.entries(item.counts).every(([letter, count]) => (available[letter] || 0) >= count)) found.push(item.word);
        }
      }
      return found;
    },
  });
}
