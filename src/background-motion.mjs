const clamp=(value,min,max)=>Math.max(min,Math.min(max,value));
const delta=(value,base)=>((value-base+540)%360)-180;
export function createTiltModel({now=()=>globalThis.performance?.now?.()??Date.now()}={}){
 let neutral,last,settling=false,movedAt=0,lastTime=0;
 return {reset(){neutral=null;last=null;settling=false;},sample(beta,gamma,angle=0){
  if(!Number.isFinite(beta)||!Number.isFinite(gamma))return null;
  const time=now();
  if(!neutral){neutral={beta,gamma};last={beta,gamma};lastTime=movedAt=time;return {x:0,y:0};}
  const dt=Math.max(0,Math.min(100,time-lastTime));lastTime=time;
  if(Math.abs(delta(beta,last.beta))+Math.abs(delta(gamma,last.gamma))>.5)movedAt=time;
  last={beta,gamma};
  // Move the neutral angle along the boundary: changing grip never leaves the wallpaper stuck.
  const dx=delta(gamma,neutral.gamma),dy=delta(beta,neutral.beta);
  if(Math.abs(dx)>22){neutral.gamma+=dx-Math.sign(dx)*22;settling=true;}
  if(Math.abs(dy)>22){neutral.beta+=dy-Math.sign(dy)*22;settling=true;}
  // After a new grip rests at the boundary, ease the neutral point into that grip.
  if(settling&&time-movedAt>500){const mix=1-Math.exp(-dt/600);neutral.gamma+=delta(gamma,neutral.gamma)*mix;neutral.beta+=delta(beta,neutral.beta)*mix;if(Math.abs(delta(gamma,neutral.gamma))+Math.abs(delta(beta,neutral.beta))<.05)settling=false;}
  const x=clamp(delta(gamma,neutral.gamma)/22,-1,1),y=clamp(delta(beta,neutral.beta)/22,-1,1),r=angle*Math.PI/180;
  return {x:clamp(x*Math.cos(r)+y*Math.sin(r),-1,1),y:clamp(y*Math.cos(r)-x*Math.sin(r),-1,1)};
 }};
}
export function createBackgroundMotion(layer,{native,app,win=window,doc=document}={}){
 const model=createTiltModel(),reduced=win.matchMedia('(prefers-reduced-motion: reduce)'),mobile=win.matchMedia('(pointer: coarse)');
 let desired=false,active=false,frame=0,x=0,y=0,targetX=0,targetY=0,lastTime=0,nativeHandle,generation=0,permission=false;
 const paint=()=>{layer.style.transform=`translate3d(${x.toFixed(2)}px,${y.toFixed(2)}px,0)`;};
 function tick(time){frame=0;const mix=1-Math.exp(-Math.min(50,lastTime?time-lastTime:16)/95);lastTime=time;x+=(targetX-x)*mix;y+=(targetY-y)*mix;paint();if(active&&(Math.abs(targetX-x)+Math.abs(targetY-y)>.03))frame=win.requestAnimationFrame(tick);}
 function sample(value){if(!active)return;const point=model.sample(value.beta,value.gamma,win.screen?.orientation?.angle??win.orientation??0);if(!point)return;targetX=-point.x*18;targetY=-point.y*18;if(!frame){lastTime=0;frame=win.requestAnimationFrame(tick);}}
 function stop(){active=false;generation++;win.removeEventListener('deviceorientation',sample);win.removeEventListener('pointerdown',authorize);win.removeEventListener('keydown',authorize);if(frame)win.cancelAnimationFrame(frame);frame=0;x=y=targetX=targetY=0;model.reset();paint();layer.style.willChange='auto';nativeHandle?.remove();nativeHandle=null;if(native)Promise.resolve(native.stop()).catch(()=>{});}
 async function authorize(){if(!active||permission)return;try{permission=(await win.DeviceOrientationEvent.requestPermission())==='granted';if(permission&&active)win.addEventListener('deviceorientation',sample);}catch{} }
 function start(){if(active)return;active=true;model.reset();layer.style.willChange='transform';const current=++generation;
  if(native){Promise.resolve(native.addListener('orientation',sample)).then(async handle=>{if(!active||current!==generation){handle.remove();return;}nativeHandle=handle;await native.start();}).catch(()=>{});}
  else if(typeof win.DeviceOrientationEvent?.requestPermission==='function'&&!permission){win.addEventListener('pointerdown',authorize);win.addEventListener('keydown',authorize);}
  else win.addEventListener('deviceorientation',sample);
 }
 function refresh(){if(desired&&!doc.hidden&&!reduced.matches&&(native||mobile.matches))start();else stop();}
 const reorient=()=>{model.reset();x=y=targetX=targetY=0;paint();};
 doc.addEventListener('visibilitychange',refresh);win.addEventListener('orientationchange',reorient);win.screen?.orientation?.addEventListener('change',reorient);reduced.addEventListener('change',refresh);mobile.addEventListener('change',refresh);
 app?.addListener('appStateChange',({isActive})=>{if(isActive)refresh();else stop();}).catch(()=>{});
 return {setEnabled(value){desired=!!value;refresh();}};
}
