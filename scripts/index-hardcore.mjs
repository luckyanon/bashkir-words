// Precompute eligibility so opening the game never scans every rack on a phone.
import {readFile,writeFile} from 'node:fs/promises';
import {createLexicon} from '../src/dictionary-service.mjs';
const url=name=>new URL('../public/data/'+name,import.meta.url);
const [raw,full,data]=await Promise.all(['game-lexicon.nouns.json','nouns.full.json','level-pools.json'].map(async name=>JSON.parse(await readFile(url(name),'utf8'))));
const lexicon=createLexicon(raw,full);
for(const pool of data.pools)pool.hardcoreWordCount=lexicon.findWords(pool.letters,{minLength:2,targetsOnly:true}).length;
await writeFile(url('level-pools.json'),JSON.stringify(data));
console.log(JSON.stringify({eligible:data.pools.filter(p=>p.hardcoreWordCount>=30).length,max:Math.max(...data.pools.map(p=>p.hardcoreWordCount))}));
