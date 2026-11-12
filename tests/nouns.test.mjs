import test from 'node:test';import assert from 'node:assert/strict';import {readFile} from 'node:fs/promises';
import {createLexicon} from '../src/dictionary-service.mjs';import {createLevelProvider,getRackCategory,maxWordLengthForLevel} from '../src/level-generator.mjs';
import {createGame} from '../src/game.mjs';import {validateLevel,getWordCells} from '../src/levels.mjs';import {signature} from '../src/lib/dictionary.mjs';import {migrateNounSave} from '../src/noun-migration.mjs';
const load=async n=>JSON.parse(await readFile(new URL('../public/data/'+n,import.meta.url),'utf8'));
const [raw,full,pools]=await Promise.all(['game-lexicon.nouns.json','nouns.full.json','level-pools.json'].map(load));const lexicon=createLexicon(raw,full);
const policy=JSON.parse(await readFile(new URL('../scripts/dictionary-policy.json',import.meta.url),'utf8'));
test('all accepted words come from supplied noun entries, and original meanings and IDs remain intact',()=>{
 assert.equal(lexicon.dictionary.size,6527);assert.equal(raw.maxLength,12);assert(raw.words.every(w=>w.length<=12));
 assert.deepEqual(raw.words.sort(),full.entries.map(e=>e.display).sort());
 for(const entry of full.entries){assert(entry.partsOfSpeech.includes('noun'));assert.equal(lexicon.getEntry(entry.display),entry);for(const meaning of [...(entry.definitions.ru||[]),...(entry.definitions.en||[])])assert.equal(meaning.partOfSpeech,'noun');}
 for(const word of raw.targetWords)assert(lexicon.getMeanings(word).length);
});
test('every source rack creates a valid field with eight to twelve targets, a full-rack word and at most three initial bonuses',()=>{
 for(const pool of pools.pools){
  const provider=createLevelProvider({...pools,pools:[pool]},lexicon,{applyLevelDifficulty:false});
  const level=provider.getLevel(121),report=validateLevel(level,lexicon);
  assert(report.valid,report.errors.join('; '));assert(level.words.length>=8&&level.words.length<=12);
  assert(level.letters.length>=3);assert(level.bonusWords.length<=3);
  assert(level.words.some(p=>p.word.length===level.letters.length&&signature(p.word)===signature(level.letters.join(''))));
 }
});
test('new difficulty stages respect 20,40,60,80,100 boundaries and sample every eligible rack size deterministically',()=>{
 assert.deepEqual([1,19,20,39,40,59,60,79,80,99,100,1000000].map(id=>maxWordLengthForLevel(id)),[6,6,7,7,8,8,9,9,10,10,12,12]);
 const provider=createLevelProvider(pools,lexicon,{seed:'noun-stage'}),reload=createLevelProvider(pools,lexicon,{seed:'noun-stage'});
 for(const [first,last,minimum,maximum] of [[1,19,3,6],[20,39,3,7],[40,59,3,8],[60,79,3,9],[80,99,4,10],[100,130,4,12]]){
  const seen=new Set();for(let id=first;id<=last;id++){
   const level=provider.getLevel(id);seen.add(level.letters.length);assert(level.letters.length>=minimum&&level.letters.length<=maximum);
   assert.deepEqual(level,reload.getLevel(id));assert(validateLevel(level,lexicon).valid);assert(level.words.length>=8&&level.words.length<=12);
  }
  const eligible=[...new Set(pools.pools.map(p=>p.letters.length))].filter(n=>n>=minimum&&n<=maximum);
  assert.deepEqual([...seen].sort((a,b)=>a-b),eligible.sort((a,b)=>a-b));
 }
 assert.deepEqual(provider.getLevel(1000000),reload.getLevel(1000000));
});
test('removed words are absent from targets, bonuses and detailed entries',()=>{
 for(const word of new Set([...policy.excludedWords,...Object.keys(policy.abbreviations),...Object.keys(policy.excludedEthnonyms)])){assert(!lexicon.hasWord(word),word);assert.equal(lexicon.getEntry(word),undefined);assert(!raw.targetWords.includes(word));for(const pool of pools.pools){assert(!pool.words.some(p=>p.word===word));assert(!pool.bonusWords.includes(word));}}
});

test('ҡайсы is a playable translated noun, while ordinary short words remain valid',()=>{
 assert(lexicon.hasWord('ҡайсы'));assert(lexicon.isTargetCandidate('ҠАЙСЫ'));
 assert.equal(lexicon.getMeanings('ҡайсы')[0].text,'ножницы');
 assert.equal(lexicon.getEntry('ҡайсы').signature,signature('ҠАЙСЫ'));
 assert(pools.pools.some(pool=>pool.words.some(word=>word.word==='ҠАЙСЫ')));
 for(const word of ['АЙ','АТ','ИТ','УТ','ИҢЕШ','ҠЫҪҠАРТМА'])assert(lexicon.hasWord(word));
});

test('replacement preserves device economy, level, gift and settings; completion and repeated words cannot farm rewards',()=>{
 const provider=createLevelProvider(pools,lexicon,{seed:'nouns-progress'});const initial=createGame(provider,lexicon).snapshot();
 const old={...initial,generatorVersion:5,currentLevel:40,maxUnlocked:40,completedRanges:[[1,39]],coins:70,settings:{sound:false,animations:true},dailyGift:{lastClaimDay:Math.floor(Date.UTC(2026,9,7)/86400000),count:1},levels:{40:{revealed:['0,0'],solved:[],bonus:[]}}};
 const migrated=migrateNounSave(old,provider);assert(migrated.changed);const game=createGame(provider,lexicon,migrated.saved);assert.equal(game.state.coins,70);assert.equal(game.state.currentLevel,40);assert.equal(game.state.settings.sound,false);assert.deepEqual(game.state.dailyGift,old.dailyGift);
 const level=game.getCurrentLevel();const target=level.words[0].word;game.submit(target);const snapshot=game.snapshot();assert.equal(game.submit(target).type,'repeat');assert.deepEqual(game.snapshot(),snapshot);
 assert.deepEqual(createGame(provider,lexicon,migrateNounSave(snapshot,provider).saved).snapshot(),snapshot);
 const fresh=createGame(provider,lexicon);assert.equal(fresh.state.currentLevel,1);assert.equal(fresh.state.coins,90);assert.equal(fresh.getLevelState().revealed.length,0);
});



