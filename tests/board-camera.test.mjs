import test from 'node:test';
import assert from 'node:assert/strict';
import {createBoardCamera} from '../src/board-camera.mjs';
const dimensions={width:360,height:300,boardWidth:1500,boardHeight:1300,overview:true};
const close=(a,b)=>assert(Math.abs(a-b)<1e-7,`${a} != ${b}`);

test('one finger moves in both axes, and release/cancel stops movement',()=>{
 const camera=createBoardCamera(dimensions),before=camera.state;
 camera.down(1,{x:150,y:120});camera.move(1,{x:180,y:160});
 close(camera.state.x,before.x+30);close(camera.state.y,before.y+40);
 camera.up(1);const after=camera.state;camera.move(1,{x:100,y:100});assert.deepEqual(camera.state,after);
 camera.down(2,{x:100,y:100});camera.cancel();camera.move(2,{x:80,y:30});assert.deepEqual(camera.state,after);
});

test('pinch preserves the point between fingers; lifting one finger continues dragging without a jump',()=>{
 const camera=createBoardCamera(dimensions),before=camera.state;
 const world={x:(150-before.x)/before.scale,y:(120-before.y)/before.scale};
 camera.down(1,{x:100,y:120});camera.down(2,{x:200,y:120});
 camera.move(1,{x:50,y:120});camera.move(2,{x:250,y:120});
 close(camera.state.scale,before.scale*2);close(camera.state.x+world.x*camera.state.scale,150);close(camera.state.y+world.y*camera.state.scale,120);
 camera.up(2);const after=camera.state;camera.move(1,{x:60,y:135});close(camera.state.x,after.x+10);close(camera.state.y,after.y+15);
});

test('zoom is anchored, bounded, and cannot lose the field outside the viewport',()=>{
 const camera=createBoardCamera(dimensions),before=camera.state,anchor={x:110,y:140};
 camera.zoom(1.25,anchor);close((anchor.x-camera.state.x)/camera.state.scale,(anchor.x-before.x)/before.scale);
 camera.zoom(100);assert.equal(camera.state.scale,camera.state.maxScale);
 camera.zoom(.000001);assert.equal(camera.state.scale,camera.state.minScale);
 assert(camera.state.x<=dimensions.width-48);assert(camera.state.x+dimensions.boardWidth*camera.state.scale>=48);
 camera.down(1,{x:0,y:0});camera.move(1,{x:50000,y:-50000});close(camera.state.x,dimensions.width-48);close(camera.state.y,48-dimensions.boardHeight*camera.state.scale);
});

test('a field smaller than the screen can move and found words focus above the stationary controls',()=>{
 const camera=createBoardCamera({width:390,height:844,boardWidth:300,boardHeight:500}),before=camera.state;
 camera.down(1,{x:150,y:150});camera.move(1,{x:160,y:175});close(camera.state.x,before.x+10);close(camera.state.y,before.y+25);camera.up(1);
 camera.reveal({x:100,y:450,width:36,height:36},{x:0,y:90,width:390,height:340});
 assert(camera.state.y+450*camera.state.scale<430);
});

test('resize preserves the viewed centre and reveal brings offscreen words into view without changing zoom',()=>{
 const camera=createBoardCamera(dimensions),before=camera.state;
 camera.resize({...dimensions,width:400,height:320});close((200-camera.state.x)/camera.state.scale,(180-before.x)/before.scale);
 const scale=camera.state.scale;camera.reveal({x:1300,y:1000,width:140,height:36});assert.equal(camera.state.scale,scale);
 const rect=camera.state;assert(rect.x+1300*scale>=0);assert(rect.y+1000*scale>=0);
});
