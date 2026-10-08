import test from 'node:test';import assert from 'node:assert/strict';import {readFile} from 'node:fs/promises';
import {createLexicon} from '../src/dictionary-service.mjs';import {createLevelProvider,getRackCategory,maxWordLengthForLevel} from '../src/level-generator.mjs';
import {createGame} from '../src/game.mjs';import {validateLevel,getWordCells} from '../src/levels.mjs';import {signature} from '../src/lib/dictionary.mjs';import {migrateNounSave} from '../src/noun-migration.mjs';
const load=async n=>JSON.parse(await readFile(new URL('../public/data/'+n,import.meta.url),'utf8'));
const [raw,full,pools]=await Promise.all(['game-lexicon.nouns.json','nouns.full.json','level-pools.json'].map(load));const lexicon=createLexicon(raw,full);
test('all accepted words come from supplied noun entries, and original meanings and IDs remain intact',()=>{
 assert.equal(lexicon.dictionary.size,6704);assert.equal(raw.maxLength,12);assert(raw.words.every(w=>w.length<=12));
 assert.deepEqual(raw.words.sort(),full.entries.map(e=>e.display).sort());
 for(const entry of full.entries){assert(entry.partsOfSpeech.includes('noun'));assert.equal(lexicon.getEntry(entry.display),entry);for(const meaning of [...(entry.definitions.ru||[]),...(entry.definitions.en||[])])assert.equal(meaning.partOfSpeech,'noun');}
 for(const word of raw.targetWords)assert(lexicon.getMeanings(word).length);
});
test('every noun pool creates a valid board with seven targets, no more than three bonuses and a full-rack word',()=>{
 const reps=[5,6,7,8].map(n=>pools.pools.find(p=>Math.min(8,p.letters.length)===n));
 for(const pool of pools.pools){const category=Math.min(8,pool.letters.length),data={...pools,pools:reps.map(p=>Math.min(8,p.letters.length)===category?pool:p)};
  const provider=createLevelProvider(data,lexicon,{applyLevelDifficulty:false}),id=Array.from({length:20},(_,i)=>121+i).find(i=>getRackCategory(i)===category),level=provider.getLevel(id),report=validateLevel(level,lexicon);
  assert.equal(report.valid,true,report.errors.join('; '));assert(level.words.length>=7);assert(level.bonusWords.length<=3);
  assert(level.words.some(p=>p.word.length===level.letters.length && signature(p.word)===signature(level.letters.join(''))));
 }
});
test('stage boundaries are 20, 50, 70, 100 and weighted decks remain deterministic at distant levels',()=>{
 assert.deepEqual([1,19,20,49,50,69,70,99,100,1000000].map(id=>maxWordLengthForLevel(id)),[8,8,9,9,10,10,11,11,12,12]);
 const provider=createLevelProvider(pools,lexicon,{seed:'noun-stage'}),reload=createLevelProvider(pools,lexicon,{seed:'noun-stage'});let largest=0;
 for(const id of [...Array.from({length:400},(_,i)=>i+1),1000000,1000001]){const level=provider.getLevel(id);assert(level.letters.length<=maxWordLengthForLevel(id));assert([...level.words.map(w=>w.word),...level.bonusWords].every(w=>w.length>=(id>80?4:id>40?3:2)));assert.equal(level.letters.length,Math.max(...level.words.map(w=>w.word.length)));assert.deepEqual(level,reload.getLevel(id));largest=Math.max(largest,level.letters.length);}
 for(let id=401;id<=6000&&largest<12;id++)if(getRackCategory(id,'noun-stage')===8)largest=Math.max(largest,provider.getLevel(id).letters.length);
 assert.equal(largest,12);
 for(const first of [1,41,81,121,1000001]){const counts={5:0,6:0,7:0,8:0};for(let id=first;id<first+20;id++)counts[Math.min(8,provider.getLevel(id).letters.length)]++;assert.deepEqual(counts,first>80?{5:0,6:16,7:3,8:1}:{5:8,6:8,7:3,8:1});}
});
test('replacement preserves device economy, level, gift and settings; completion and repeated words cannot farm rewards',()=>{
 const provider=createLevelProvider(pools,lexicon,{seed:'nouns-progress'});const initial=createGame(provider,lexicon).snapshot();
 const old={...initial,generatorVersion:5,currentLevel:40,maxUnlocked:40,completedRanges:[[1,39]],coins:70,settings:{sound:false,animations:true},dailyGift:{lastClaimDay:Math.floor(Date.UTC(2026,9,7)/86400000),count:1},levels:{40:{revealed:['0,0'],solved:[],bonus:[]}}};
 const migrated=migrateNounSave(old,provider);assert(migrated.changed);const game=createGame(provider,lexicon,migrated.saved);assert.equal(game.state.coins,70);assert.equal(game.state.currentLevel,40);assert.equal(game.state.settings.sound,false);assert.deepEqual(game.state.dailyGift,old.dailyGift);
 const level=game.getCurrentLevel();const target=level.words[0].word;game.submit(target);const snapshot=game.snapshot();assert.equal(game.submit(target).type,'repeat');assert.deepEqual(game.snapshot(),snapshot);
 assert.deepEqual(createGame(provider,lexicon,migrateNounSave(snapshot,provider).saved).snapshot(),snapshot);
 const fresh=createGame(provider,lexicon);assert.equal(fresh.state.currentLevel,1);assert.equal(fresh.state.coins,90);assert.equal(fresh.getLevelState().revealed.length,0);
});



