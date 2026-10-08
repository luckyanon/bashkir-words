import {getWordCells} from './levels.mjs';
export function migrateLocalWords(saved,oldProvider,provider){
 if(!saved||saved.generatorVersion!==6||(saved.wordRevision===provider.wordRevision && (saved.difficultyRevision||0)===(provider.difficultyRevision||0)))return {saved,changed:false};
 const result=structuredClone(saved);result.wordRevision=provider.wordRevision;result.difficultyRevision=provider.difficultyRevision;
 for(const [id,progress] of Object.entries(result.levels||{})){
  const old=oldProvider.getLevel(Number(id)),next=provider.getLevel(Number(id));
  if(JSON.stringify(old.words)===JSON.stringify(next.words))continue;
  const shown=new Set(progress.revealed||[]),positions=new Map();
  for(const word of old.words)positions.set(word.word,getWordCells(word).map(cell=>shown.has(cell.key)));
  const revealed=new Set();for(const word of next.words)getWordCells(word).forEach((cell,index)=>{if(positions.get(word.word)?.[index])revealed.add(cell.key);});
  progress.revealed=[...revealed];progress.solved=[];
 }
 return {saved:result,changed:true};
}

