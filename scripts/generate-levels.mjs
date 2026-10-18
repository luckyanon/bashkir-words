import {readFile,writeFile} from 'node:fs/promises';
import {createLexicon} from '../src/dictionary-service.mjs';
import {signature} from '../src/lib/dictionary.mjs';
import {createLevelProvider,minWordLengthForRack} from '../src/level-generator.mjs';
import {validateLevel} from '../src/levels.mjs';
const url=name=>new URL('../public/data/'+name,import.meta.url);
const raw=JSON.parse(await readFile(url('game-lexicon.nouns.json'))),full=JSON.parse(await readFile(url('nouns.full.json')));
const lexicon=createLexicon(raw,full),pools=[],seen=new Set();
for(const fullWord of raw.targetWords.filter(w=>w.length>=3)){
 const rack=signature(fullWord);if(seen.has(rack))continue;seen.add(rack);
 const minWordLength=minWordLengthForRack(rack.length),buildable=lexicon.findWords(rack,{minLength:minWordLength});
 const targets=buildable.filter(lexicon.isTargetCandidate);if(targets.length<8)continue;
 const candidates=targets.filter(w=>w!==fullWord).sort((a,b)=>a.length-b.length||a.localeCompare(b));
 // Retain a full-rack noun and spread the other targets across available lengths.
 const selected=[fullWord];const count=Math.min(11,candidates.length);
 for(let i=0;i<count;i++)selected.push(candidates[Math.floor(i*candidates.length/count)]);
 const bonusWords=buildable.filter(w=>!lexicon.isTargetCandidate(w)).slice(0,3);
 pools.push({letters:[...rack],words:selected.map(word=>({word,entryId:lexicon.getEntry(word).id})),bonusWords,minWordLength,excludedWordLengths:Array.from({length:minWordLength-2},(_,i)=>i+2),hardcoreWordCount:lexicon.findWords(rack,{minLength:2,targetsOnly:true}).length});
}
const data={schemaVersion:'6.0.0',datasetRevision:raw.datasetRevision,generatorVersion:6,language:'ba',partOfSpeech:'noun',maxWordLength:raw.maxLength,minTargetWords:8,maxTargetWords:12,maxBonusWords:3,pools};
const provider=createLevelProvider(data,lexicon);const examples=Array.from({length:32},(_,i)=>provider.getLevel(i+1));
for(const level of examples){const report=validateLevel(level,lexicon);if(!report.valid)throw Error(report.errors.join('\n'));}
await writeFile(url('level-pools.json'),JSON.stringify(data));await writeFile(url('levels.json'),JSON.stringify(examples));
console.log(JSON.stringify({poolCount:pools.length,distribution:Object.fromEntries([...new Set(pools.map(p=>p.letters.length))].sort((a,b)=>a-b).map(n=>[n,pools.filter(p=>p.letters.length===n).length]))}));

