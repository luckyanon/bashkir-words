import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createLexicon} from '../src/dictionary-service.mjs';
import {createLevelProvider} from '../src/level-generator.mjs';
import {createGame,BONUS_REWARD,INITIAL_COINS} from '../src/game.mjs';
const load=async name=>JSON.parse(await readFile(new URL('../public/data/'+name,import.meta.url),'utf8'));
const [raw,full,data]=await Promise.all(['game-lexicon.nouns.json','nouns.full.json','level-pools.json'].map(load));
const lexicon=createLexicon(raw,full),provider=createLevelProvider(data,lexicon,{seed:'classic-bonus-tests'});
let level,extras;
for(let id=1;id<=100;id++){
 const candidate=provider.getLevel(id),words=new Set(candidate.words.map(p=>p.word));
 const omitted=lexicon.findWords(candidate.letters,{minLength:candidate.minWordLength,targetsOnly:true}).filter(w=>!words.has(w));
 if(omitted.length>5){level=candidate;extras=omitted;break;}
}
assert(level);

test('all translated nouns omitted from a classic field are playable bonuses; targets and length rules stay intact',()=>{
 const game=createGame([level],lexicon);
 for(const word of extras){const result=game.submit(word);assert.equal(result.type,'bonus',word);assert.equal(result.reward,BONUS_REWARD);}
 assert.equal(game.state.coins,INITIAL_COINS+extras.length*BONUS_REWARD);
 assert.deepEqual(game.getLevelState().bonus,extras);assert.equal(game.getLevelState().solved.length,0);
 for(const target of level.words)assert(['target','repeat'].includes(game.submit(target.word).type));
 const short=lexicon.findWords(level.letters,{maxLength:level.minWordLength-1})[0];
 if(short)assert.equal(game.submit(short).type,'excluded-length');
});

test('bonus repeats identify the notification without paying again, including after reload and replay',()=>{
 const game=createGame([level],lexicon);
 for(const word of extras)game.submit(word);
 const snapshot=game.snapshot(),restored=createGame([level],lexicon,snapshot);
 assert.deepEqual(restored.snapshot(),snapshot);assert(restored.state.bonusRanges.length>3);
 for(const word of extras){const result=restored.submit(word.toLowerCase());assert.equal(result.type,'repeat');assert.equal(result.bonus,true);assert.equal(result.reward,0);}
 assert.equal(restored.state.coins,snapshot.coins);
 restored.replay(level.id);for(const word of extras)assert.equal(restored.submit(word).reward,0);
 assert.equal(restored.state.coins,snapshot.coins);
});

test('old three-slot saves keep their original bonus rewards and can earn omitted nouns afterwards',()=>{
 const game=createGame([level],lexicon);for(const word of level.bonusWords)game.submit(word);
 const saved=game.snapshot();assert.equal(saved.bonusRanges.length,3);
 const restored=createGame([level],lexicon,saved);assert.equal(restored.state.coins,saved.coins);
 for(const word of level.bonusWords)assert.equal(restored.submit(word).reward,0);
 assert.equal(restored.submit(extras[0]).reward,BONUS_REWARD);
 assert.equal(restored.state.coins,saved.coins+BONUS_REWARD);
 assert.equal(restored.getLevelState().bonus.includes(null),false);
});
