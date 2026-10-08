import test from 'node:test';import assert from 'node:assert/strict';import {readFile} from 'node:fs/promises';
import {BACKGROUNDS,availableBackground,backgroundRewardsForLevel} from '../src/backgrounds.mjs';
import {createLexicon} from '../src/dictionary-service.mjs';import {createLevelProvider} from '../src/level-generator.mjs';import {createGame} from '../src/game.mjs';import {getWordCells} from '../src/levels.mjs';import {migrateLocalWords} from '../src/local-word-migration.mjs';
const load=async name=>JSON.parse(await readFile(new URL('../public/data/'+name,import.meta.url),'utf8'));
const [raw,full,pools]=await Promise.all(['game-lexicon.nouns.json','nouns.full.json','level-pools.json'].map(load));const lexicon=createLexicon(raw,full);
test('backgrounds open at exactly 1,20,40,60,80; rewards appear when next level unlocks',()=>{
 assert.deepEqual(BACKGROUNDS.map(bg=>bg.level),[1,20,40,60,80]);
 for(const bg of BACKGROUNDS.slice(1)){assert.equal(availableBackground(bg.id,bg.level-1),undefined);assert.equal(availableBackground(bg.id,bg.level),bg);assert.deepEqual(backgroundRewardsForLevel(bg.level-1),[bg]);assert.deepEqual(backgroundRewardsForLevel(bg.level),[]);}
});
test('locked backgrounds cannot be selected and an unlocked selection survives restart',()=>{
 const provider=createLevelProvider(pools,lexicon),fresh=createGame(provider,lexicon);fresh.setSettings({background:'olasay'});assert.equal(fresh.state.settings.background,undefined);
 const initial=fresh.snapshot(),game=createGame(provider,lexicon,{...initial,currentLevel:80,maxUnlocked:80,completedRanges:[[1,79]]});game.setSettings({background:'olasay'});assert.equal(game.state.settings.background,'olasay');assert.equal(createGame(provider,lexicon,game.snapshot()).state.settings.background,'olasay');game.reset();assert.equal(game.state.settings.background,undefined);
});
test('new background reward survives restart and cannot be awarded again by replay',()=>{
 const provider=createLevelProvider(pools,lexicon),initial=createGame(provider,lexicon).snapshot();const game=createGame(provider,lexicon,{...initial,currentLevel:19,maxUnlocked:19,completedRanges:[[1,18]]});for(const word of game.getCurrentLevel().words)game.submit(word.word);assert.deepEqual(game.getBackgroundRewards().map(bg=>bg.id),['salavat']);const restored=createGame(provider,lexicon,game.snapshot());assert.deepEqual(restored.getBackgroundRewards().map(bg=>bg.id),['salavat']);restored.dismissBackgroundRewards(['salavat']);restored.replay(19);for(const word of restored.getCurrentLevel().words)restored.submit(word.word);assert.deepEqual(restored.getBackgroundRewards(),[]);
});
test('BAL is a translated field target whenever allowed and buildable, never a bonus',()=>{
 assert.equal(lexicon.getMeanings('бал')[0].text,'мёд');const provider=createLevelProvider(pools,lexicon,{seed:'bal-check'});let seen=0;
 for(let id=1;id<=100;id++){const level=provider.getLevel(id);if(level.minWordLength<=3&&lexicon.canBuild('БАЛ',level.letters)){seen++;assert(level.words.some(w=>w.word==='БАЛ'));assert(!level.bonusWords.includes('БАЛ'));const game=createGame([level],lexicon);assert.equal(game.submit('бал').type,'target');assert.equal(game.submit('бал').type,'repeat');}else assert(!level.words.some(w=>w.word==='БАЛ'));}assert(seen>0);
});
test('adding BAL preserves solved words, paid hint information, coins and gift on changed boards',()=>{
 const options={seed:'bal-migration'},balPool=pools.pools.find(p=>p.letters.length===5&&lexicon.canBuild('БАЛ',p.letters)),data={...pools,pools:[balPool,...[6,7,8].map(n=>pools.pools.find(p=>Math.min(8,p.letters.length)===n))]},old=createLevelProvider(data,lexicon,{...options,includeLocalWords:false}),next=createLevelProvider(data,lexicon,options);let id=1;while(id<300&&!next.getLevel(id).words.some(w=>w.word==='БАЛ'))id++;assert(id<300);
 const initial=createGame(old,lexicon).snapshot(),game=createGame(old,lexicon,{...initial,currentLevel:id,maxUnlocked:id,completedRanges:id>1?[[1,id-1]]:[]});game.submit(game.getCurrentLevel().words[0].word);game.hint();game.claimDailyGift();const saved=game.snapshot(),migrated=migrateLocalWords(saved,old,next);assert(migrated.changed);const restored=createGame(next,lexicon,migrated.saved);assert.equal(restored.state.coins,saved.coins);assert.deepEqual(restored.state.dailyGift,saved.dailyGift);assert.equal(restored.state.hintCount,saved.hintCount);
 for(const word of old.getLevel(id).words){const shown=getWordCells(word).map(cell=>saved.levels[id].revealed.includes(cell.key));const placement=next.getLevel(id).words.find(item=>item.word===word.word);getWordCells(placement).forEach((cell,index)=>{if(shown[index])assert(restored.getLevelState().revealed.includes(cell.key));});}
 assert.equal(migrateLocalWords(restored.snapshot(),old,next).changed,false);
});

