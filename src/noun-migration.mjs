import {getWordCells} from './levels.mjs';
export function migrateNounSave(saved,provider){
 if(!saved || (saved.generatorVersion===provider.generatorVersion && String(saved.datasetRevision||"")===String(provider.datasetRevision||"")))return {saved,changed:false};
 const result=structuredClone(saved);result.generatorVersion=provider.generatorVersion;result.datasetRevision=provider.datasetRevision;result.layoutVersion=provider.layoutVersion;result.wordRevision=provider.wordRevision;result.difficultyRevision=provider.difficultyRevision;result.legacyLayoutThroughLevel=provider.legacyLayoutThroughLevel;result.legacyThroughLevel=0;result.legacyV3ThroughLevel=0;delete result.rackRepairVersion;
 for(const [key,old] of Object.entries(result.levels || {})){
  const id=Number(key);if(!provider.hasLevel(id)){delete result.levels[key];continue;}
  const level=provider.getLevel(id),solved=new Set(old.solved || []),completed=(result.completedRanges || []).some(([a,b])=>id>=a&&id<=b),shown=new Set();
  for(const placement of level.words)if(completed || solved.has(placement.word))for(const cell of getWordCells(placement))shown.add(cell.key);
  result.levels[key]={...old,revealed:[...shown],solved:[],bonus:(old.bonus || []).filter(word=>level.bonusWords.includes(word))};
 }
 return {saved:result,changed:true};
}



