import test from 'node:test';import assert from 'node:assert/strict';import {createTiltModel,createBackgroundMotion} from '../src/background-motion.mjs';import {createGame} from '../src/game.mjs';
test('a large change of grip moves the neutral point, settles at centre and responds to a new small tilt',()=>{
 let time=0;const model=createTiltModel({now:()=>time});model.sample(0,0);time=33;
 assert.equal(model.sample(90,0).y,1);let point;
 for(let i=0;i<120;i++){time+=33;point=model.sample(90,0);}
 assert(Math.abs(point.y)<.01);time+=33;assert(model.sample(79,0).y<-.45);
});
test('tilt calibrates to the current holding angle, caps movement and handles angle wrap',()=>{
 const model=createTiltModel();assert.equal(model.sample(null,0),null);assert.deepEqual(model.sample(45,10),{x:0,y:0});assert.deepEqual(model.sample(67,32),{x:1,y:1});assert.deepEqual(model.sample(-100,-100),{x:-1,y:-1});model.reset();model.sample(179,0);assert(Math.abs(model.sample(-179,0).y-2/22)<1e-8);
});
test('tilt follows screen rotation and recalibrates after resume',()=>{
 const model=createTiltModel();model.sample(0,0);const portrait=model.sample(0,22),landscape=model.sample(0,22,90);assert.equal(portrait.x,1);assert(Math.abs(landscape.x)<1e-8);assert.equal(landscape.y,-1);model.reset();assert.deepEqual(model.sample(45,20),{x:0,y:0});
});

test('background sensor stops when disabled or hidden, recalibrates and honours reduced motion',()=>{
 const win=new EventTarget(),doc=new EventTarget(),frames=new Map(),queries=new Map();doc.hidden=false;let next=0;
 win.matchMedia=query=>{if(!queries.has(query)){const value=new EventTarget();value.matches=query.includes('coarse');queries.set(query,value);}return queries.get(query);};
 win.screen={orientation:Object.assign(new EventTarget(),{angle:0})};win.requestAnimationFrame=callback=>{frames.set(++next,callback);return next;};win.cancelAnimationFrame=id=>frames.delete(id);
 const layer={style:{}},motion=createBackgroundMotion(layer,{win,doc}),sample=(beta,gamma)=>win.dispatchEvent(Object.assign(new Event('deviceorientation'),{beta,gamma}));
 motion.setEnabled(true);sample(40,0);sample(50,20);const callback=[...frames.values()].at(-1);frames.clear();callback(16);assert.notEqual(layer.style.transform,'translate3d(0.00px,0.00px,0)');
 motion.setEnabled(false);assert.equal(frames.size,0);assert.equal(layer.style.transform,'translate3d(0.00px,0.00px,0)');sample(80,30);assert.equal(frames.size,0);
 motion.setEnabled(true);doc.hidden=true;doc.dispatchEvent(new Event('visibilitychange'));assert.equal(frames.size,0);doc.hidden=false;doc.dispatchEvent(new Event('visibilitychange'));sample(80,30);assert.equal(layer.style.transform,'translate3d(0.00px,0.00px,0)');
 const reduced=win.matchMedia('(prefers-reduced-motion: reduce)');reduced.matches=true;reduced.dispatchEvent(new Event('change'));sample(90,40);assert.equal(frames.size,0);
});
