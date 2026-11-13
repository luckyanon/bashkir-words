import {createLexicon} from './dictionary-service.mjs';
import {createLevelProvider} from './level-generator.mjs';
import {classicBonusWords, alignBonusWords} from './game.mjs';
import {getWordCells} from './levels.mjs';
import {layoutOptionsForSave} from './layout-migration.mjs';

export const PREVIOUS_DATASET = 'nouns-2026-10-19-rules-2';
const HISTORY_FILES = {
  [PREVIOUS_DATASET]: '2.9.0.json',
  'nouns-2026-11-01-review-1': '2.9.4.json',
  '3e289d7b8e8e501e': '2.9.5.json',
  '0f4edc6e8c823fa9': '2.10.0.json',
  '6987d31ccbc93291': '2.10.2.json',
  'dd0523ec354e3366': '2.10.3.json',
};
const validThrough = value => Number.isSafeInteger(value) && value > 0;
function savedLayouts(saved) {
  if (Array.isArray(saved?.bonusLayouts)) return saved.bonusLayouts.filter(item =>
    item && HISTORY_FILES[item.datasetRevision] && validThrough(item.throughLevel)).slice(0, 8);
  return validThrough(saved?.bonusLayoutThroughLevel)
    ? [{datasetRevision: PREVIOUS_DATASET, throughLevel: saved.bonusLayoutThroughLevel}] : [];
}
export function releaseHistoryFiles(saved) {
  if (saved?.generatorVersion !== 6) return [];
  return [...new Set([saved.datasetRevision, ...savedLayouts(saved).map(item => item.datasetRevision)]
    .map(revision => HISTORY_FILES[revision]).filter(Boolean))];
}
export const needsReleaseHistory = saved => releaseHistoryFiles(saved).length > 0;

// Archives only interpret old coordinates and reward slots. They never become
// the playable dictionary. Multiple updates compose their slot layouts in order.
export function restoreRelease(saved, provider, lexicon, history) {
  if (!needsReleaseHistory(saved)) return {saved, provider, changed:false};
  const histories = new Map((history.histories || [history]).map(item => [item.raw.datasetRevision, item]));
  const cache = new Map();
  function previous(revision) {
    if (!cache.has(revision)) {
      const data = histories.get(revision);
      if (!data) throw new Error('Missing dictionary migration history: ' + revision);
      const oldLexicon = createLexicon(data.raw, {entries:[...lexicon.entriesById.values(), ...data.removedEntries]});
      cache.set(revision, {lexicon:oldLexicon,
        provider:createLevelProvider(data.pools, oldLexicon, {seed:provider.seed, ...layoutOptionsForSave(saved,6)}),
        hardcore:() => createLevelProvider(data.pools, oldLexicon, {seed:provider.seed+':hardcore',hardcore:true})});
    }
    return cache.get(revision);
  }
  const layouts = savedLayouts(saved).map(item => ({...item}));
  const migrating = !!HISTORY_FILES[saved.datasetRevision] && saved.datasetRevision !== provider.datasetRevision;
  if (migrating) {
    const throughLevel = Math.max(saved.currentLevel||1, saved.maxUnlocked||1);
    const last = layouts.at(-1);
    if (last?.datasetRevision === saved.datasetRevision) last.throughLevel = Math.max(last.throughLevel, throughLevel);
    else layouts.push({datasetRevision:saved.datasetRevision, throughLevel});
  }
  const wrapped = {...provider, bonusLayouts:layouts, legacyBonusWords(id) {
    let slots = null;
    for (const layout of layouts) if (id <= layout.throughLevel) {
      const old = previous(layout.datasetRevision);
      const words = classicBonusWords(old.provider.getLevel(id), old.lexicon);
      slots = slots ? alignBonusWords(slots, words) : words;
    }
    return slots;
  }};
  if (!migrating) return {saved, provider:wrapped, changed:false};
  const old = previous(saved.datasetRevision), result = structuredClone(saved);
  function remap(progress, before, after) {
    if (!progress || JSON.stringify(before.words) === JSON.stringify(after.words)) return;
    // Old releases did not record chronology. Retain their available order
    // before clearing solved coordinates; the game filters it after remapping.
    if (!Array.isArray(progress.foundOrder)) progress.foundOrder = [...(progress.solved || []), ...(progress.bonus || [])];
    const shown = new Set(progress.revealed||[]), positions = new Map();
    for (const word of before.words) positions.set(word.word, getWordCells(word).map(cell => shown.has(cell.key)));
    const revealed = new Set();
    for (const word of after.words) getWordCells(word).forEach((cell,i) => {
      if (positions.get(word.word)?.[i]) revealed.add(cell.key);
    });
    // Compensate paid letters on removed/replaced words without losing hints.
    const desired = Math.min(progress.completed&&progress.rewarded ? Infinity : shown.size,
      new Set(after.words.flatMap(word => getWordCells(word).map(cell => cell.key))).size);
    for (const word of after.words) for (const cell of getWordCells(word)) if (revealed.size < desired) revealed.add(cell.key);
    progress.revealed = [...revealed]; progress.solved = [];
  }
  for (const [id, progress] of Object.entries(result.levels||{})) {
    if (provider.hasLevel(Number(id))) remap(progress, old.provider.getLevel(Number(id)), provider.getLevel(Number(id)));
  }
  if (result.hardcore) {
    const before = old.hardcore();
    const after = createLevelProvider(history.currentPools, lexicon, {seed:provider.seed+':hardcore',hardcore:true});
    const id = result.hardcore.currentLevel;
    remap(result.hardcore.progress, before.getLevel(id), after.getLevel(id));
    result.hardcore.datasetRevision = provider.datasetRevision;
  }
  result.datasetRevision = provider.datasetRevision;
  result.bonusLayouts = layouts;
  delete result.bonusLayoutThroughLevel;
  return {saved:result, provider:wrapped, changed:true};
}
