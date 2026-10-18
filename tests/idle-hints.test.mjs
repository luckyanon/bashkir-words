import test from 'node:test';import assert from 'node:assert/strict';
import {createIdleHintClock,hintTileIds} from '../src/idle-hints.mjs';
test('idle hints alternate at 30 and 60 seconds, last seven seconds and never overlap',()=>{
 let time=0;const clock=createIdleHintClock({now:()=>time,random:()=>.99}),words=['БАЛ','АЛМА'];
 time=29999;assert.equal(clock.tick(words),null);time=30000;const first=clock.tick(words);assert.equal(first.type,'translation');assert.equal(first.word,'АЛМА');
 time=36999;assert.equal(clock.tick(words),first);time=37000;assert.equal(clock.tick(words),null);
 time=59999;assert.equal(clock.tick(words),null);time=60000;assert.equal(clock.tick(words).type,'letters');
 time=67000;assert.equal(clock.tick(words),null);time=90000;assert.equal(clock.tick(words).type,'translation');
});
test('any attempt hides the current hint, resets the appropriate delay and preserves alternation',()=>{
 let time=0;const clock=createIdleHintClock({now:()=>time});
 time=29000;clock.attempt();time=58999;assert.equal(clock.tick(['БАЛ']),null);
 time=59000;assert.equal(clock.tick(['БАЛ']).type,'translation');time=60000;clock.attempt();
 time=119999;assert.equal(clock.tick(['АЛМА']),null);time=120000;assert.equal(clock.tick(['АЛМА']).type,'letters');
 clock.reset();time=150000;assert.equal(clock.tick([]),null);assert.equal(clock.tick(['АЛМА']).type,'translation');
});
test('random candidate selection uses only the supplied unsolved targets; duplicate letters need separate tiles',()=>{
 for(const random of [()=>0,()=>.9999]){let time=0;const clock=createIdleHintClock({now:()=>time,random});time=30000;assert(['БАЛ','АЛМА'].includes(clock.tick(['БАЛ','АЛМА']).word));}
 const tiles=[... 'АЛААМ'].map((letter,id)=>({letter,id}));
 assert.deepEqual(hintTileIds('АЛМА',tiles),[0,1,2,4]);assert.deepEqual(hintTileIds('БАЛ',tiles),[]);
});
