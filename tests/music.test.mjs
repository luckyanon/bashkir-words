import test from 'node:test';
import assert from 'node:assert/strict';
import {createMusic,createKuraiInstrument,KURAI_SCORE,MUSIC_BEAT} from '../src/music.mjs';

function audioFixture({deferResume=false}={}) {
  const counters={resumed:0,started:0,stopped:0,closed:0,disconnected:0};
  const sources=[];let finishResume;
  const parameter=()=>({value:0,setValueAtTime(){},linearRampToValueAtTime(){},exponentialRampToValueAtTime(){}});
  const node=()=>({connect(){},disconnect(){counters.disconnected++;}});
  const source=()=>{const s={...node(),frequency:parameter(),detune:parameter(),setPeriodicWave(){},start(){counters.started++;},stop(){counters.stopped++;}};sources.push(s);return s;};
  class Context {
    currentTime=0;sampleRate=100;destination={};
    createGain(){return {...node(),gain:parameter()};}
    createOscillator(){return source();}
    createBufferSource(){return source();}
    createPeriodicWave(){return {};}
    createBuffer(_,size){const data=new Float32Array(size);return {getChannelData:()=>data};}
    createBiquadFilter(){return {...node(),frequency:parameter(),Q:parameter()};}
    resume(){counters.resumed++;return deferResume?new Promise(resolve=>{finishResume=resolve;}):Promise.resolve();}
    async suspend(){}async close(){counters.closed++;}
  }
  return {Context,counters,sources,finish:()=>finishResume?.()};
}

test('music unlocks once, stops when muted or backgrounded and resumes independently of effects',async()=>{
  const win=new EventTarget(),doc=new EventTarget();doc.hidden=false;
  const {Context,counters:c}=audioFixture();const music=createMusic({win,doc,AudioContext:Context});
  try{
    assert.equal(c.started,0);win.dispatchEvent(new Event('pointerdown'));win.dispatchEvent(new Event('keydown'));await Promise.resolve();assert.equal(c.resumed,1);assert(c.started>0);
    music.setEnabled(false);assert(c.stopped>=c.started);const paused=c.started;win.dispatchEvent(new Event('pointerdown'));await Promise.resolve();assert.equal(c.started,paused);
    music.setEnabled(true);await Promise.resolve();assert(c.started>paused);music.setActive(false);const background=c.started;
    win.dispatchEvent(new Event('pointerdown'));await Promise.resolve();assert.equal(c.started,background);
    music.setActive(true);await Promise.resolve();assert(c.started>background);
    doc.hidden=true;doc.dispatchEvent(new Event('visibilitychange'));const hidden=c.started;await Promise.resolve();assert.equal(c.started,hidden);
  }finally{music.dispose();}assert.equal(c.closed,1);
});

test('disposing while audio resumes cannot start voices or restart playback',async()=>{
  const win=new EventTarget(),doc=new EventTarget();doc.hidden=false;
  const {Context,counters:c,finish}=audioFixture({deferResume:true});const music=createMusic({win,doc,AudioContext:Context});
  win.dispatchEvent(new Event('pointerdown'));music.dispose();finish();await Promise.resolve();
  assert.equal(c.started,0);music.setEnabled(true);music.setActive(true);win.dispatchEvent(new Event('keydown'));await Promise.resolve();
  assert.equal(c.resumed,1);assert.equal(c.closed,1);
});

test('breath, tone and vibrato release all audio nodes on note end and stop',()=>{
  const {Context,counters:c,sources}=audioFixture(),ctx=new Context(),instrument=createKuraiInstrument(ctx,ctx.destination);
  instrument.play(0,0,1);assert.equal(sources.length,0);
  instrument.play(74,.1,2,71);assert.equal(c.started,3);
  for(const source of sources)source.onended();assert.equal(c.disconnected,7);
  instrument.stop();assert.equal(c.disconnected,7);
  instrument.play(78,3,1);instrument.stop();assert.equal(c.disconnected,14);
  for(const source of sources.slice(3))source.onended();assert.equal(c.disconnected,14);
  assert(KURAI_SCORE.reduce((sum,[,beats])=>sum+beats*MUSIC_BEAT,0)>50);
});
