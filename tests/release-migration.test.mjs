import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createLexicon} from '../src/dictionary-service.mjs';
import {createLevelProvider} from '../src/level-generator.mjs';
import {createGame,classicBonusWords} from '../src/game.mjs';
import {restoreRelease,needsReleaseHistory,releaseHistoryFiles} from '../src/release-migration.mjs';
import {createStorage,STORAGE_KEY} from '../src/storage.mjs';
const load=async name=>JSON.parse(await readFile(new URL('../public/data/'+name,import.meta.url),'utf8'));
const [raw,full,pools,history,reviewHistory,latestHistory,unslopHistory]=await Promise.all(['game-lexicon.nouns.json','nouns.full.json','level-pools.json','migrations/2.9.0.json','migrations/2.9.4.json','migrations/2.9.5.json','migrations/2.10.0.json'].map(load));
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


test('2.9.5 update preserves purchases, paid hints and bonus identity across direct and chained upgrades',()=>{
 const resources={histories:[history,reviewHistory,latestHistory],currentPools:pools};
 const previousLexicon=createLexicon(latestHistory.raw,{entries:[...full.entries,...latestHistory.removedEntries]});
 for(const chainLength of [0,1,2]){
  const seed='release-2.10-'+chainLength;
  const current=createLevelProvider(pools,lexicon,{seed});
  const hardcore=createLevelProvider(pools,lexicon,{seed:seed+':hardcore',hardcore:true});
  const previous=createLevelProvider(latestHistory.pools,previousLexicon,{seed});
  const previousHard=createLevelProvider(latestHistory.pools,previousLexicon,{seed:seed+':hardcore',hardcore:true});
  let priorProvider=previous, initial=null;
  if(chainLength){
   const origin=createLevelProvider(history.pools,oldLexicon,{seed});
   const oldGame=createGame(origin,oldLexicon);
   initial={...oldGame.snapshot(),currentLevel:120,maxUnlocked:120,completedRanges:[[1,119]],coins:5000};
   if(chainLength===2){
    const middleLexicon=createLexicon(reviewHistory.raw,{entries:[...full.entries,...reviewHistory.removedEntries]});
    const middle=createLevelProvider(reviewHistory.pools,middleLexicon,{seed});
    const step=restoreRelease(initial,middle,middleLexicon,{...resources,currentPools:reviewHistory.pools});
    initial=createGame(step.provider,middleLexicon,step.saved).snapshot();
   }
   const migrated=restoreRelease(initial,previous,previousLexicon,{...resources,currentPools:latestHistory.pools});
   priorProvider=migrated.provider;initial=migrated.saved;
  }else initial={...createGame(previous,previousLexicon).snapshot(),currentLevel:120,maxUnlocked:120,completedRanges:[[1,119]],coins:5000};
  const game=createGame(priorProvider,previousLexicon,initial,{hardcoreSource:previousHard});
  game.purchaseBackground('salavat');game.claimDailyGift();game.setSettings({music:false,sound:false});
  for(const word of classicBonusWords(game.getCurrentLevel(),previousLexicon).filter(Boolean))game.submit(word);
  game.hint();game.hint();game.setMode('hardcore');game.submit(game.getCurrentLevel().words[0].word);
  const saved=game.snapshot();assert(releaseHistoryFiles(saved).includes('2.9.5.json'));
  let migration=restoreRelease(saved,current,lexicon,resources);
  assert(migration.changed);assert.equal(migration.saved.bonusLayouts.length,chainLength+1);
  for(let reload=0;reload<3;reload++){
   const restored=createGame(migration.provider,lexicon,migration.saved,{hardcoreSource:hardcore});
   assert.equal(restored.state.coins,saved.coins);assert.equal(restored.state.currentLevel,120);
   assert.equal(restored.state.maxUnlocked,120);assert.equal(restored.state.hintCount,saved.hintCount);
   assert.deepEqual(restored.state.dailyGift,saved.dailyGift);assert.deepEqual(restored.state.backgrounds,saved.backgrounds);
   assert.deepEqual(restored.state.settings,saved.settings);assert.deepEqual(restored.state.bonusRanges,saved.bonusRanges);
   assert(restored.state.hardcore.progress.revealed.length>=Math.min(saved.hardcore.progress.revealed.length,restored.getBoard().length));
   restored.setMode('classic');
   if(!reload)assert(restored.getLevelState().revealed.length>=Math.min(saved.levels[120].revealed.length,restored.getBoard().length));
   restored.replay(120);
   for(const word of saved.levels[120].bonus){
    if(classicBonusWords(restored.getCurrentLevel(),lexicon).includes(word))assert(restored.state.bonusRewards.includes('120:'+word),word);
    const before=restored.state.coins;restored.submit(word);assert.equal(restored.state.coins,before);
   }
   for(const entry of latestHistory.removedEntries)assert(!lexicon.hasWord(entry.display),entry.display);
   restored.setMode('hardcore');migration=restoreRelease(restored.snapshot(),current,lexicon,resources);
   assert.equal(migration.changed,false);
  }
 }
});

test('each release archive retains every formerly translated target after later removals',()=>{
 for(const archive of [history,reviewHistory,latestHistory,unslopHistory]){
  const historical=createLexicon(archive.raw,{entries:[...full.entries,...archive.removedEntries]});
  assert.equal(historical.targetWords.length,archive.raw.targetWords.length,archive.raw.datasetRevision);
  for(const word of archive.raw.words)assert(historical.getEntry(word),'Missing historical entry: '+word);
 }
});

test('2.10.1 preserves changed boards and reward identities after direct and full-chain upgrades',()=>{
 const archives=[history,reviewHistory,latestHistory,unslopHistory];
 const resources={histories:archives,currentPools:pools};
 const historicalLexicon=archive=>createLexicon(archive.raw,{entries:[...full.entries,...archive.removedEntries]});
 const removed=new Set(unslopHistory.removedEntries.map(entry=>entry.display));
 let replayedBonuses=0,freshBonuses=0;
 for(const chain of [[unslopHistory],[latestHistory,unslopHistory],archives]){
  const seed='release-2.10.1-chain-1';
  const current=createLevelProvider(pools,lexicon,{seed});
  const hard=createLevelProvider(pools,lexicon,{seed:seed+':hardcore',hardcore:true});
  const previousLexicon=historicalLexicon(unslopHistory);
  const previous=createLevelProvider(unslopHistory.pools,previousLexicon,{seed});
  let id;
  for(let candidate=120;candidate<320;candidate++){
   const before=previous.getLevel(candidate),after=current.getLevel(candidate);
   const finalBonuses=classicBonusWords(after,lexicon).filter(Boolean);
   const common=classicBonusWords(before,previousLexicon).filter(word=>word&&finalBonuses.includes(word));
   if(before.words.some(word=>removed.has(word.word))&&common.length>=2&&finalBonuses.length>=3){id=candidate;break;}
  }
  assert(id,'Need a changed board with surviving bonus words for chain '+chain.length);
  const firstLexicon=historicalLexicon(chain[0]);
  const first=createLevelProvider(chain[0].pools,firstLexicon,{seed});
  const firstHard=createLevelProvider(chain[0].pools,firstLexicon,{seed:seed+':hardcore',hardcore:true});
  const initial={...createGame(first,firstLexicon).snapshot(),currentLevel:id,maxUnlocked:id,
   completedRanges:[[1,id-1]],coins:5000};
  let game=createGame(first,firstLexicon,initial,{hardcoreSource:firstHard});
  game.purchaseBackground('salavat');game.claimDailyGift();game.setSettings({music:false,sound:false,backgroundMotion:false});
  for(let index=0;index<chain.length;index++){
   const archive=chain[index],stageLexicon=historicalLexicon(archive);
   const stageBonuses=classicBonusWords(game.getCurrentLevel(),stageLexicon).filter(Boolean);
   const finalBonuses=classicBonusWords(current.getLevel(id),lexicon).filter(Boolean);
   const bonus=stageBonuses.find(word=>finalBonuses.includes(word))||stageBonuses[0];
   assert(bonus);game.submit(bonus);game.hint();
   game.setMode('hardcore');game.submit(game.getCurrentLevel().words[0].word);
   const saved=game.snapshot(),serialized=JSON.stringify(saved);
   const nextArchive=chain[index+1],nextLexicon=nextArchive?historicalLexicon(nextArchive):lexicon;
   const nextPools=nextArchive?.pools||pools;
   const next=createLevelProvider(nextPools,nextLexicon,{seed});
   const nextHard=createLevelProvider(nextPools,nextLexicon,{seed:seed+':hardcore',hardcore:true});
   const migration=restoreRelease(saved,next,nextLexicon,{...resources,currentPools:nextPools});
   assert(migration.changed);assert.equal(JSON.stringify(saved),serialized,'Migration must not mutate backup source');
   game=createGame(migration.provider,nextLexicon,migration.saved,{hardcoreSource:nextHard});
   assert.equal(game.state.coins,saved.coins);assert.equal(game.state.hintCount,saved.hintCount);
   assert.deepEqual(game.state.bonusRanges,saved.bonusRanges);
   assert.deepEqual(game.state.dailyGift,saved.dailyGift);assert.deepEqual(game.state.backgrounds,saved.backgrounds);
   assert.deepEqual(game.state.settings,saved.settings);assert.equal(game.state.currentLevel,id);
   assert.equal(game.state.maxUnlocked,id);assert.equal(game.state.mode,'hardcore');
   assert(game.getLevelState().revealed.length>=Math.min(saved.hardcore.progress.revealed.length,game.getBoard().length));
   game.setMode('classic');
   assert(game.getLevelState().revealed.length>=Math.min(saved.levels[id].revealed.length,game.getBoard().length));
  }
  assert.equal(game.state.bonusLayouts.length,chain.length);
  assert(releaseHistoryFiles(game.snapshot()).includes('2.10.0.json'));
  for(const word of removed)assert(!lexicon.hasWord(word),'Removed headword '+word);
  const paid=game.state.bonusRewards.filter(key=>key.startsWith(id+':')).map(key=>key.slice(String(id).length+1));
  assert(paid.length,'At least one previously paid bonus must survive');
  for(let reload=0;reload<3;reload++){
   const snapshot=game.snapshot();
   const migration=restoreRelease(snapshot,current,lexicon,resources);
   assert.equal(migration.changed,false);
   game=createGame(migration.provider,lexicon,migration.saved,{hardcoreSource:hard});
   assert.equal(game.state.coins,snapshot.coins);assert.deepEqual(game.state.bonusRanges,snapshot.bonusRanges);
   game.replay(id);
   for(const word of paid){
    const coins=game.state.coins,result=game.submit(word);
    assert(['bonus','repeat'].includes(result.type),word+' remains a bonus');
    assert.equal(game.state.coins,coins);assert.equal(result.reward||0,0);replayedBonuses++;
   }
   if(!reload){
    const fresh=classicBonusWords(game.getCurrentLevel(),lexicon).find(word=>word&&!game.state.bonusRewards.includes(id+':'+word));
    if(fresh){const coins=game.state.coins,result=game.submit(fresh);assert.equal(result.type,'bonus');assert.equal(result.reward,5);assert.equal(game.state.coins,coins+5);freshBonuses++;paid.push(fresh);}
   }
  }
 }
 assert(replayedBonuses>=9,'Every chain checks paid bonuses after reload and replay');
 assert(freshBonuses>0,'New bonus slots remain payable after preserving old slots');
});
