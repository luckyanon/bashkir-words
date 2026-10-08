import test from 'node:test';import assert from 'node:assert/strict';import {readFile} from 'node:fs/promises';
import {BACKGROUNDS,availableBackground} from '../src/backgrounds.mjs';
import {createLexicon} from '../src/dictionary-service.mjs';import {createLevelProvider} from '../src/level-generator.mjs';import {createGame} from '../src/game.mjs';import {getWordCells} from '../src/levels.mjs';import {migrateLocalWords} from '../src/local-word-migration.mjs';
const load=async name=>JSON.parse(await readFile(new URL('../public/data/'+name,import.meta.url),'utf8'));
const [raw,full,pools]=await Promise.all(['game-lexicon.nouns.json','nouns.full.json','level-pools.json'].map(load));const lexicon=createLexicon(raw,full);
test('background shop has the requested prices and only Ufa is free',()=>{
 assert.deepEqual(BACKGROUNDS.map(bg=>bg.price),[0,1000,1600,2400,3000]);
 assert.equal(availableBackground('ufa'),BACKGROUNDS[0]);assert.equal(availableBackground('salavat'),undefined);
 const fresh=createGame(createLevelProvider(pools,lexicon),lexicon);assert.deepEqual(fresh.state.backgrounds.owned,['ufa']);
 fresh.setSettings({background:'olasay'});assert.equal(fresh.state.settings.background,undefined);
});
const fundedGame=()=>{const provider=createLevelProvider(pools,lexicon),initial=createGame(provider,lexicon).snapshot();return {provider,game:createGame(provider,lexicon,{...initial,currentLevel:300,completedRanges:[[1,299]],coins:90+299*30})};};
test('all backgrounds can be bought once, applied and restored without a second debit',()=>{
 const {provider,game}=fundedGame();let coins=game.state.coins;
 for(const bg of BACKGROUNDS.slice(1)){
  const before=game.snapshot();assert.equal(game.ownsBackground(bg.id),false);assert.deepEqual(game.snapshot(),before);
  assert.deepEqual(game.purchaseBackground(bg.id),{type:'purchased',background:bg.id,cost:bg.price});coins-=bg.price;
  assert.equal(game.state.coins,coins);assert.equal(game.state.settings.background,bg.id);assert(game.ownsBackground(bg.id));
  assert.equal(game.purchaseBackground(bg.id).type,'owned');assert.equal(game.state.coins,coins);
  const restored=createGame(provider,lexicon,game.snapshot());assert.equal(restored.state.coins,coins);assert(restored.ownsBackground(bg.id));assert.equal(restored.state.settings.background,bg.id);
 }
 const restored=createGame(provider,lexicon,game.snapshot());restored.setSettings({background:'ufa'});assert.equal(restored.state.coins,coins);assert.equal(restored.state.settings.background,'ufa');
 // Restoring an inflated balance cannot refund purchases.
 const forged={...restored.snapshot(),coins:90+299*30};assert.equal(createGame(provider,lexicon,forged).state.coins,coins);
});
test('insufficient coins and unknown backgrounds cannot change coins or ownership',()=>{
 const game=createGame(createLevelProvider(pools,lexicon),lexicon),before=game.snapshot();
 assert.equal(game.purchaseBackground('salavat').type,'insufficient');assert.equal(game.purchaseBackground('unknown').type,'invalid');assert.equal(game.purchaseBackground('ufa').type,'owned');assert.deepEqual(game.snapshot(),before);
});
test('reaching background milestones no longer grants backgrounds or victory rewards',()=>{
 const provider=createLevelProvider(pools,lexicon),initial=createGame(provider,lexicon).snapshot();
 for(const id of [19,39,59,79]){const game=createGame(provider,lexicon,{...initial,currentLevel:id,completedRanges:[[1,id-1]]});for(const word of game.getCurrentLevel().words)game.submit(word.word);assert.equal(game.getMaxUnlocked(),id+1);assert.deepEqual(game.state.backgrounds.owned,['ufa']);assert.equal(game.state.pendingBackgrounds,undefined);assert.deepEqual(createGame(provider,lexicon,game.snapshot()).state.backgrounds.owned,['ufa']);}
});
test('pre-shop earned backgrounds survive migration once, with no charge and no future unlocks',()=>{
 const provider=createLevelProvider(pools,lexicon),old=createGame(provider,lexicon).snapshot();delete old.backgrounds;
 Object.assign(old,{currentLevel:20,completedRanges:[[1,19]],coins:450,pendingBackgrounds:['salavat']});old.settings.background='salavat';
 const game=createGame(provider,lexicon,old);assert.deepEqual(game.state.backgrounds.owned,['ufa','salavat']);assert.deepEqual(game.state.backgrounds.paid,[]);assert.equal(game.state.coins,450);assert.equal(game.state.settings.background,'salavat');assert.equal(game.state.pendingBackgrounds,undefined);
 const later={...game.snapshot(),currentLevel:80,completedRanges:[[1,79]]};const restored=createGame(provider,lexicon,later);assert.deepEqual(restored.state.backgrounds.owned,['ufa','salavat']);assert.equal(restored.ownsBackground('olasay'),false);assert.equal(restored.state.coins,450);
});
test('reset removes purchased backgrounds and restores default Ufa without refund',()=>{
 const {game}=fundedGame();game.purchaseBackground('olasay');game.setSettings({sound:false});game.reset();assert.equal(game.state.coins,90);assert.deepEqual(game.state.backgrounds,{version:1,owned:['ufa'],paid:[]});assert.equal(game.state.settings.background,undefined);assert.equal(game.state.settings.sound,false);
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

