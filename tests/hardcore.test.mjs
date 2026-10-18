import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createLexicon} from '../src/dictionary-service.mjs';
import {createLevelProvider} from '../src/level-generator.mjs';
import {createGame,HINT_COST,LEVEL_REWARD,BONUS_REWARD,HARDCORE_WORD_REWARD,HARDCORE_LEVEL_REWARD} from '../src/game.mjs';
import {validateLevel,getWordCells} from '../src/levels.mjs';
import {signature} from '../src/lib/dictionary.mjs';
import {createStorage,createWebStorageAdapter} from '../src/storage.mjs';

const load=async name=>JSON.parse(await readFile(new URL('../public/data/'+name,import.meta.url),'utf8'));
const [raw,full,data]=await Promise.all(['game-lexicon.nouns.json','nouns.full.json','level-pools.json'].map(load));
const lexicon=createLexicon(raw,full),reps=[5,6,7,8].map(n=>data.pools.find(p=>Math.min(8,p.letters.length)===n));
function forPool(pool,seed='hardcore-test'){
 const category=Math.min(8,pool.letters.length),pools={...data,pools:reps.map(p=>Math.min(8,p.letters.length)===category?pool:{...p,hardcoreWordCount:0})};
 const provider=createLevelProvider(pools,lexicon,{hardcore:true,seed,applyLevelDifficulty:false});
 return provider.getLevel(121);
}
function engines(seed='hardcore-save'){
 return {normal:createLevelProvider(data,lexicon,{seed}),hardcore:createLevelProvider(data,lexicon,{seed:seed+':hardcore',hardcore:true})};
}
function gameForLevel(level){
 const {normal,hardcore}=engines('hardcore-specific-field'),source={...hardcore,getLevel:id=>id===level.id?level:hardcore.getLevel(id)};
 const game=createGame(normal,lexicon,null,{hardcoreSource:source});game.state.hardcore.currentLevel=level.id;game.setMode('hardcore');return game;
}

test('every hardcore rack includes all eligible translated nouns and every placement is valid',()=>{
 for(const pool of data.pools.filter(p=>p.hardcoreWordCount>=30)){
  const level=forPool(pool),report=validateLevel(level,lexicon);
  assert(report.valid,`${pool.words[0].word}: ${report.errors.join('; ')}`);
  assert.deepEqual(level.words.map(w=>w.word).sort(),lexicon.findWords(level.letters,{minLength:level.minWordLength}).filter(lexicon.isTargetCandidate).sort());
  assert(level.bonusWords.length<=3);assert.equal(Math.max(...level.words.map(w=>w.word.length)),level.letters.length);
  assert(level.words.length>=30);assert.equal(level.minWordLength,2);assert.deepEqual(level.excludedWordLengths,[]);assert.equal(level.words.length,pool.hardcoreWordCount);
 }
});

test('largest field fits every target, and SARA and KISHER are accepted on formerly omitted racks',()=>{
 const pool=data.pools.reduce((largest,p)=>p.hardcoreWordCount>largest.hardcoreWordCount?p:largest),largest=forPool(pool);
 assert.equal(largest.words.length,183);assert(largest.rows>24||largest.cols>24);
 assert(largest.words.some(w=>w.word.length===2));assert(largest.words.some(w=>w.word.length===3));
 assert(!validateLevel({...largest,words:largest.words.slice(0,29)},lexicon).valid);
 const game=gameForLevel(largest);for(const target of largest.words)assert(['target','repeat'].includes(game.submit(target.word).type));
 assert(game.getLevelState().completed);assert.equal(game.state.coins,90+largest.words.length*6);
 for(const word of ['САРА','КИШЕР']){
  const pool=data.pools.find(p=>p.hardcoreWordCount>=30&&lexicon.canBuild(word,p.letters)&&!p.words.some(w=>w.word===word));
  assert(pool);const level=forPool(pool);assert(level.words.some(w=>w.word===word));assert(!level.bonusWords.includes(word));
  assert.equal(createGame([level],lexicon).submit(word.toLowerCase()).type,'target');
 }
});

test('eligibility index matches the dictionary; every stage selects 30 or more words with short words enabled',()=>{
 for(const pool of data.pools)assert.equal(pool.hardcoreWordCount,lexicon.findWords(pool.letters,{minLength:2,targetsOnly:true}).length);
 const {hardcore}=engines('hardcore-stages');
 for(const id of [...Array.from({length:40},(_,i)=>i+1),41,49,50,69,70,79,80,81,99,100,120,500,10000]){
  const level=hardcore.getLevel(id);assert(level.words.length>=30);assert.equal(level.minWordLength,2);assert.deepEqual(level.excludedWordLengths,[]);
  const report=validateLevel(level,lexicon);assert(report.valid,report.errors.join('; '));
 }
});

test('upgrading hardcore rules remaps solved words rather than reusing coordinates from the old field',()=>{
 const {normal,hardcore}=engines('hardcore-upgrade'),game=createGame(normal,lexicon,null,{hardcoreSource:hardcore});
 game.submit(game.getCurrentLevel().words[0].word);const classic=structuredClone(game.state.levels);game.setMode('hardcore');
 const word=game.getCurrentLevel().words[0].word;game.submit(word);const saved=game.snapshot();
 saved.hardcore.rulesRevision=1;saved.hardcore.progress.revealed=game.getBoard().map(cell=>cell.key);
 const restored=createGame(normal,lexicon,saved,{hardcoreSource:hardcore});
 assert(restored.getLevelState().solved.includes(word));assert(!restored.getLevelState().completed);
 assert.equal(restored.state.coins,saved.coins);assert.deepEqual(restored.state.levels,classic);assert.equal(restored.state.hardcore.rulesRevision,2);
});

test('hardcore persists independently, shares coins/settings and regenerates without altering the classic board',async()=>{
 const {normal,hardcore}=engines(),game=createGame(normal,lexicon,null,{hardcoreSource:hardcore});
 game.submit(game.getCurrentLevel().words[0].word);game.hint();const classic=structuredClone(game.state.levels),classicId=game.state.currentLevel;
 game.setMode('hardcore');game.submit(game.getCurrentLevel().words[0].word);game.hint();game.claimDailyGift();game.setSettings({backgroundMotion:false,sound:false});
 const snapshot=game.snapshot(),storage=createStorage(createWebStorageAdapter(null));assert(await storage.save(snapshot));
 const restored=createGame(normal,lexicon,await storage.load(),{hardcoreSource:hardcore});assert(restored.isHardcore);assert.deepEqual(restored.snapshot(),snapshot);
 const rack=signature(restored.rack.join('')),coins=restored.state.coins;assert(restored.regenerateHardcore());assert.notEqual(signature(restored.rack.join('')),rack);
 assert.equal(restored.getLevelState().revealed.length,0);assert.equal(restored.state.coins,coins);assert.deepEqual(restored.state.levels,classic);
 restored.startLevel(classicId);assert(!restored.isHardcore);assert.deepEqual(restored.state.levels,classic);assert.equal(restored.state.coins,coins);assert.equal(restored.state.settings.sound,false);
 const savedHardcore=restored.state.hardcore.currentLevel;restored.setMode('hardcore');assert.equal(restored.getCurrentLevel().id,savedHardcore);
 restored.reset();assert(!restored.isHardcore);assert.equal(restored.state.currentLevel,1);assert.equal(restored.state.hardcore.currentLevel,1);assert.equal(restored.state.coins,90);
});

test('hardcore pays the exact completion formula once, preserves saves, and blocks hints',()=>{
 const {normal,hardcore}=engines('hardcore-economy'),game=createGame(normal,lexicon,null,{hardcoreSource:hardcore});game.setMode('hardcore');
 for(let i=0;i<20&&!game.getCurrentLevel().bonusWords.length;i++)game.regenerateHardcore();
 const level=game.getCurrentLevel(),before=game.state.coins;assert.equal(game.hint().type,'unavailable');assert.equal(game.state.coins,before);
 let bonuses=0;for(const word of level.bonusWords){const result=game.submit(word);assert.equal(result.type,'bonus');assert.equal(result.reward,0);bonuses++;const coins=game.state.coins;assert.equal(game.submit(word).type,'repeat');assert.equal(game.state.coins,coins);}
 const completions=[];
 for(const word of level.words){const result=game.submit(word.word);if(result.type==='target')assert.equal(result.reward,result.newlySolved.length*HARDCORE_WORD_REWARD);if(result.completionReward)completions.push(result.completionReward);}
 assert.deepEqual(completions,[level.words.length*6+bonuses*12]);assert.equal(game.state.hardcore.wordCount,level.words.length);
 assert(game.getLevelState().completed);assert.equal(game.state.coins,before+level.words.length*6+bonuses*12);assert.equal(game.getMaxUnlocked(),1);assert.deepEqual(game.state.backgrounds.owned,['ufa']);
 const saved=game.snapshot(),restored=createGame(normal,lexicon,saved,{hardcoreSource:hardcore});assert.deepEqual(restored.snapshot(),saved);
 for(const word of level.words)assert.equal(restored.submit(word.word).type,'repeat');assert.equal(restored.state.coins,saved.coins);
 restored.regenerateHardcore();assert.equal(restored.state.coins,saved.coins);assert.equal(restored.getLevelState().rewarded,false);assert.deepEqual(restored.state.hardcore.rewardedWords,[]);
 assert.equal(createGame(normal,lexicon,restored.snapshot(),{hardcoreSource:hardcore}).state.coins,saved.coins);
});

test('old hardcore saves retain their balance and paid status; only future words and completions use new rewards',()=>{
 const {normal,hardcore}=engines('hardcore-reward-migration'),game=createGame(normal,lexicon,null,{hardcoreSource:hardcore});game.setMode('hardcore');
 for(const target of game.getCurrentLevel().words)game.submit(target.word);
 game.regenerateHardcore();game.submit(game.getCurrentLevel().words[0].word);
 for(const word of game.getCurrentLevel().bonusWords)game.submit(word);
 const old=game.snapshot(),h=old.hardcore;
 for(const key of ['rewardRevision','wordCount','rewardedWords','legacyCompletedCount','legacyBonusCount'])delete h[key];
 old.coins=90+h.completedCount*30+h.bonusCount*BONUS_REWARD;
 const restored=createGame(normal,lexicon,old,{hardcoreSource:hardcore});assert.equal(restored.state.coins,old.coins);
 assert.equal(restored.state.hardcore.legacyCompletedCount,h.completedCount);assert.equal(restored.state.hardcore.legacyBonusCount,h.bonusCount);
 for(const word of [...h.progress.solved,...h.progress.bonus]){assert.equal(restored.submit(word).type,'repeat');assert.equal(restored.state.coins,old.coins);}
 const remaining=restored.getCurrentLevel().words.length-h.progress.solved.length;
 for(const target of restored.getCurrentLevel().words)restored.submit(target.word);
 assert.equal(restored.state.coins,old.coins+restored.getCurrentLevel().words.length*6+h.progress.bonus.length*12);
 const again=createGame(normal,lexicon,restored.snapshot(),{hardcoreSource:hardcore});assert.deepEqual(again.snapshot(),restored.snapshot());
});
