import test from 'node:test';import assert from 'node:assert/strict';import {createMusic} from '../src/music.mjs';
test('music unlocks once, stops when muted or backgrounded and resumes independently of effects',async()=>{
 const win=new EventTarget(),doc=new EventTarget();doc.hidden=false;let resumed=0,started=0,stopped=0,closed=0;
 const parameter=()=>({value:0,setValueAtTime(){},linearRampToValueAtTime(){},exponentialRampToValueAtTime(){}});
 class Context{
  currentTime=0;destination={};
  createGain(){return {gain:parameter(),connect(){},disconnect(){}};}
  createOscillator(){return {frequency:parameter(),connect(){},disconnect(){},start(){started++;},stop(){stopped++;}};}
  async resume(){resumed++;}async suspend(){}async close(){closed++;}
 }
 const music=createMusic({win,doc,AudioContext:Context});
 assert.equal(started,0);win.dispatchEvent(new Event('pointerdown'));win.dispatchEvent(new Event('keydown'));await Promise.resolve();assert.equal(resumed,1);assert(started>0);
 music.setEnabled(false);assert(stopped>=started);const paused=started;win.dispatchEvent(new Event('pointerdown'));await Promise.resolve();assert.equal(started,paused);
 music.setEnabled(true);await Promise.resolve();assert(started>paused);music.setActive(false);const background=started;
 win.dispatchEvent(new Event('pointerdown'));await Promise.resolve();assert.equal(started,background);
 music.setActive(true);await Promise.resolve();assert(started>background);
 doc.hidden=true;doc.dispatchEvent(new Event('visibilitychange'));const hidden=started;await Promise.resolve();assert.equal(started,hidden);
 music.dispose();assert.equal(closed,1);
});
