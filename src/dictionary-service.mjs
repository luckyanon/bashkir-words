import { createDictionary, normalizeWord } from './lib/dictionary.mjs';


/** Keeps the prepared dictionary replaceable, and the original entry IDs intact. */
export function createLexicon(lexiconData, fullData, starterData = {}) {
  const additions=[];
  const dictionary = createDictionary({...lexiconData,words:[...new Set([...lexiconData.words,...additions.map(e=>e.word)])],targetWords:[...new Set([...lexiconData.targetWords,...additions.map(e=>e.word)])]});
  if (!fullData || !Array.isArray(fullData.entries)) throw new TypeError('Detailed dictionary entries are missing');
  const entries = new Map();
  const entriesById = new Map();
  for (const entry of [...fullData.entries,...additions]) {
    if (!entry || typeof entry.id !== 'string') continue;
    entriesById.set(entry.id, entry);
    for (const spelling of [entry.display, entry.word]) {
      const word = normalizeWord(spelling);
      if (word) entries.set(word, entry);
    }
  }
  const getEntry = value => entries.get(normalizeWord(value));
  const getMeanings = value => {
    const entry = typeof value === 'object' && value !== null ? value : getEntry(value);
    return (entry?.definitions?.ru || []).filter(sense => typeof sense?.text === 'string' && sense.text.trim());
  };
  const starterValues = starterData.starterWords || starterData.words ||
    (starterData.entries || []).map(entry => entry.display || entry.word);
  const starterWords = [...new Set(starterValues.map(value => normalizeWord(typeof value === 'object' ? value.display || value.word : value)))].filter(word =>
    dictionary.hasWord(word) && dictionary.isTargetCandidate(word) && getMeanings(word).length);
  const targetWords = lexiconData.targetWords.filter(word => getMeanings(word).length);
  return Object.freeze({
    dictionary, entries, entriesById, starterWords, targetWords,
    additionalWords:lexiconData.language==='ba'&&lexiconData.partOfSpeech==='noun'&&getEntry('БАЛ')?['БАЛ']:[],
    getEntry, getEntryById: id => entriesById.get(id), getMeanings,
    normalize: normalizeWord,
    hasWord: dictionary.hasWord,
    isTargetCandidate: value => dictionary.isTargetCandidate(value) && getMeanings(value).length > 0,
    canBuild: dictionary.canBuild,
    findWords: dictionary.findWords,
    metadata: { schemaVersion: lexiconData.schemaVersion, datasetVersion: lexiconData.datasetVersion, size: dictionary.size },
  });
}

/** All game and translation resources are local build assets. */
export async function loadLexicon(base = './', fetcher = globalThis.fetch, { extended = false } = {}) {
  if (typeof fetcher !== 'function') throw new TypeError('A fetch implementation is required');
  const prefix = base.endsWith('/') ? base : `${base}/`;
  const files = ['game-lexicon.nouns.json', 'nouns.full.json'];
  const data = await Promise.all(files.map(async name => {
    const response = await fetcher(`${prefix}data/${name}`);
    if (!response.ok) throw new Error(`Не удалось загрузить локальную базу: ${name}`);
    return response.json();
  }));
  return createLexicon(...data,{starterWords:data[0].starterWords || []});
}

