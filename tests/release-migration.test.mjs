import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createLexicon} from '../src/dictionary-service.mjs';
import {createLevelProvider} from '../src/level-generator.mjs';
import {createGame,classicBonusWords} from '../src/game.mjs';
import {restoreRelease,needsReleaseHistory} from '../src/release-migration.mjs';
import {createStorage,STORAGE_KEY} from '../src/storage.mjs';
const load=async name=>JSON.parse(await readFile(new URL('../public/data/'+name,import.meta.url),'utf8'));
const [raw,full,pools,history,reviewHistory]=await Promise.all(['game-lexicon.nouns.json','nouns.full.json','level-pools.json','migrations/2.9.0.json','migrations/2.9.4.json'].map(load));
history.currentPools=pools;
const lexicon=createLexicon(raw,full),oldLexicon=createLexicon(history.raw,{entries:[...full.entries,...history.removedEntries]});

test('2.9.1–2.9.4 saves retain paid hints, economy and bonus ledgers, including a prior 2.9.0 migration',()=>{
 const middleLexicon=createLexicon(reviewHistory.raw,{entries:[...full.entries,...reviewHistory.removedEntries]});
 for(const upgraded of [false,true]){
  const seed='dictionary-cleanup-'+upgraded;
  const middle=createLevelProvider(reviewHistory.pools,middleLexicon,{seed});
  const middleHard=createLevelProvider(reviewHistory.pools,middleLexicon,{seed:seed+':hardcore',hardcore:true});
  let middleProvider=middle;
  if(upgraded){
   const original=createLevelProvider(history.pools,oldLexicon,{seed});
   middleProvider={...middle,bonusLayoutThroughLevel:100,
    legacyBonusWords:id=>id<=100?classicBonusWords(original.getLevel(id),oldLexicon):null};
  }
  let game=createGame(middleProvider,middleLexicon,null,{hardcoreSource:middleHard});
  game=createGame(middleProvider,middleLexicon,{...game.snapshot(),currentLevel:120,maxUnlocked:120,
   completedRanges:[[1,119]],coins:4000},{hardcoreSource:middleHard});
  // Claim rewards both below and above the old migration boundary.
  for(const id of [100,120]){
   game.replay(id);
   for(const word of classicBonusWords(game.getCurrentLevel(),middleLexicon).filter(Boolean))game.submit(word);
  }
  game.hint();game.hint();game.setMode('hardcore');game.submit(game.getCurrentLevel().words[0].word);
  const saved=game.snapshot(),original=JSON.stringify(saved);
  const current=createLevelProvider(pools,lexicon,{seed}),hard=createLevelProvider(pools,lexicon,{seed:seed+':hardcore',hardcore:true});
  const resources={histories:[history,reviewHistory],currentPools:pools};
  let migration=restoreRelease(saved,current,lexicon,resources);
  assert(migration.changed);assert.equal(JSON.stringify(saved),original);
  assert.equal(migration.saved.bonusLayouts.length,upgraded?2:1);
  for(let reload=0;reload<3;reload++){
   const restored=createGame(migration.provider,lexicon,migration.saved,{hardcoreSource:hard});
   assert.equal(restored.state.coins,saved.coins);assert.equal(restored.state.hintCount,saved.hintCount);
   assert.deepEqual(restored.state.bonusRanges,saved.bonusRanges);
   assert(restored.state.hardcore.progress.revealed.length>=Math.min(saved.hardcore.progress.revealed.length,restored.getBoard().length));
   restored.setMode('classic');
   if(reload===0)assert(restored.getLevelState().revealed.length>=Math.min(saved.levels[120].revealed.length,restored.getBoard().length));
   for(const id of [100,120]){
    restored.replay(id);
    const paid=restored.state.bonusRewards;
    for(const word of saved.levels[id].bonus){
     if(classicBonusWords(restored.getCurrentLevel(),lexicon).includes(word)){
      assert(paid.includes(id+':'+word),id+':'+word);
      assert.equal(restored.submit(word).reward,0);
     }
    }
   }
   restored.setMode('hardcore');
   migration=restoreRelease(restored.snapshot(),current,lexicon,resources);
   assert.equal(migration.changed,false);
  }
 }
});

test('real 2.9.0 campaigns retain economy, purchases, hints, bonus identities and both modes after two reloads',()=>{
  for(const seed of ['review-a','review-b','review-c']){
    const old=createLevelProvider(history.pools,oldLexicon,{seed});
    const oldHard=createLevelProvider(history.pools,oldLexicon,{seed:seed+':hardcore',hardcore:true});
    const initial=createGame(old,oldLexicon,null,{hardcoreSource:oldHard}).snapshot();
    const game=createGame(old,oldLexicon,{...initial,currentLevel:120,maxUnlocked:120,completedRanges:[[1,119]],coins:4000},{hardcoreSource:oldHard});
    game.purchaseBackground('salavat');game.claimDailyGift();game.setSettings({music:false,sound:false});
    for(const word of classicBonusWords(game.getCurrentLevel(),oldLexicon).filter(Boolean).slice(0,4))game.submit(word);
    game.submit(game.getCurrentLevel().words[0].word);game.hint();
    game.setMode('hardcore');game.submit(game.getCurrentLevel().words[0].word);
    const saved=game.snapshot(),original=JSON.stringify(saved);
    const next=createLevelProvider(pools,lexicon,{seed}),hardcore=createLevelProvider(pools,lexicon,{seed:seed+':hardcore',hardcore:true});
    const migration=restoreRelease(saved,next,lexicon,history);
    assert(migration.changed);assert.equal(JSON.stringify(saved),original);
    let restored=createGame(migration.provider,lexicon,migration.saved,{hardcoreSource:hardcore});
    assert.equal(restored.state.currentLevel,120);assert.equal(restored.state.maxUnlocked,120);
    assert.equal(restored.state.coins,saved.coins);assert.deepEqual(restored.state.dailyGift,saved.dailyGift);
    assert.equal(restored.state.hintCount,saved.hintCount);assert.deepEqual(restored.state.backgrounds,saved.backgrounds);
    assert.deepEqual(restored.state.settings,saved.settings);assert.equal(restored.state.mode,'hardcore');
    assert(restored.state.hardcore.progress.revealed.length>=Math.min(saved.hardcore.progress.revealed.length,restored.getBoard().length));
    restored.setMode('classic');
    assert(restored.getLevelState().revealed.length>=Math.min(saved.levels[120].revealed.length,restored.getBoard().length));
    for(const word of saved.levels[120].bonus){const coins=restored.state.coins;restored.submit(word);assert.equal(restored.state.coins,coins);}
    restored.replay(120);
    for(const word of saved.levels[120].bonus){const coins=restored.state.coins;restored.submit(word);assert.equal(restored.state.coins,coins);}
    for(let i=0;i<2;i++){
      const snapshot=restored.snapshot(),reload=restoreRelease(snapshot,next,lexicon,history);
      assert.equal(reload.changed,false);assert(needsReleaseHistory(snapshot));
      restored=createGame(reload.provider,lexicon,reload.saved,{hardcoreSource:hardcore});
      assert.equal(restored.state.coins,snapshot.coins);
      for(const word of saved.levels[120].bonus){const coins=restored.state.coins;restored.submit(word);assert.equal(restored.state.coins,coins);}
    }
  }
});

test('queued saves use immutable revision metadata and read the primary save once per write',async()=>{
  const values=new Map([[STORAGE_KEY,JSON.stringify({version:2,levels:{},datasetRevision:'old'})]]);
  let reads=0;
  const storage=createStorage({get:async key=>{if(key===STORAGE_KEY)reads++;return values.get(key)??null;},set:async(k,v)=>values.set(k,v),remove:async k=>values.delete(k)});
  const state={version:2,levels:{},datasetRevision:'new',generatorVersion:6,layoutVersion:2,wordRevision:1};
  const operation=storage.save(state);state.datasetRevision='mutated';
  assert(await operation);assert.equal(reads,1);
  assert(values.has(`${STORAGE_KEY}:before-dataset-new`));assert(!values.has(`${STORAGE_KEY}:before-dataset-mutated`));
  assert.equal(JSON.parse(values.get(STORAGE_KEY)).datasetRevision,'new');
});
