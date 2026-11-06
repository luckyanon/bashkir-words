const clamp=(value,min,max)=>Math.max(min,Math.min(max,value));

/** Coordinates live in the board viewport; moving the board never scrolls the page. */
export function createBoardCamera({width,height,boardWidth,boardHeight,overview=false}){
  let x=0,y=0,scale=1,minScale,maxScale=2.5;
  const pointers=new Map();let gesture;
  const fit=()=>Math.min(1,(width-24)/boardWidth,(height-24)/boardHeight);
  function constrain(){
    const bound=(position,view,content)=>clamp(position,48-content,view-48);
    x=bound(x,width,boardWidth*scale);y=bound(y,height,boardHeight*scale);
  }
  function baseline(){
    const points=[...pointers.values()];
    if(points.length>=2){const [a,b]=points,center={x:(a.x+b.x)/2,y:(a.y+b.y)/2};gesture={distance:Math.max(1,Math.hypot(a.x-b.x,a.y-b.y)),scale,anchorX:(center.x-x)/scale,anchorY:(center.y-y)/scale};}
    else gesture=points.length?{point:points[0],x,y}:null;
  }
  const snapshot=()=>({x,y,scale,minScale,maxScale});
  function zoom(factor,anchor={x:width/2,y:height/2}){
    const next=clamp(scale*factor,minScale,maxScale),ratio=next/scale;
    x=anchor.x-(anchor.x-x)*ratio;y=anchor.y-(anchor.y-y)*ratio;scale=next;constrain();baseline();return snapshot();
  }
  minScale=Math.max(.03,fit()*.7);scale=overview?Math.max(fit(),.7):fit();
  x=(width-boardWidth*scale)/2;y=(height-boardHeight*scale)/2;constrain();
  return {
    get state(){return snapshot();},
    zoom,
    down(id,point){pointers.set(id,point);baseline();},
    move(id,point){
      if(!pointers.has(id))return snapshot();pointers.set(id,point);
      if(pointers.size>=2){const [a,b]=[...pointers.values()],center={x:(a.x+b.x)/2,y:(a.y+b.y)/2};scale=clamp(gesture.scale*Math.hypot(a.x-b.x,a.y-b.y)/gesture.distance,minScale,maxScale);x=center.x-gesture.anchorX*scale;y=center.y-gesture.anchorY*scale;}
      else if(gesture?.point){x=gesture.x+point.x-gesture.point.x;y=gesture.y+point.y-gesture.point.y;}
      constrain();return snapshot();
    },
    up(id){pointers.delete(id);baseline();},
    cancel(){pointers.clear();baseline();},
    resize(next){
      const centerX=(width/2-x)/scale,centerY=(height/2-y)/scale;
      ({width,height,boardWidth,boardHeight}=next);minScale=Math.max(.03,fit()*.7);scale=clamp(scale,minScale,maxScale);
      x=width/2-centerX*scale;y=height/2-centerY*scale;constrain();baseline();return snapshot();
    },
    reveal(rect,region={x:0,y:0,width,height}){
      const left=x+rect.x*scale,top=y+rect.y*scale,right=left+rect.width*scale,bottom=top+rect.height*scale;
      if(left<region.x+12||right>region.x+region.width-12||top<region.y+12||bottom>region.y+region.height-12){x=region.x+region.width/2-(rect.x+rect.width/2)*scale;y=region.y+region.height/2-(rect.y+rect.height/2)*scale;constrain();baseline();}
      return snapshot();
    },
  };
}

export function bindBoardCamera(viewport,board,{overview=false,rows,cols,cellSize=36,gap=3,focusArea}={}){
  const dimensions=()=>({width:viewport.clientWidth,height:viewport.clientHeight,boardWidth:cols*cellSize+(cols-1)*gap,boardHeight:rows*cellSize+(rows-1)*gap});
  const camera=createBoardCamera({...dimensions(),overview}),listeners=[],captured=new Set();
  // Coalesce input to one paint per display frame. Translation never resizes
  // the grid; zoom uses real CSS sizes so glyphs stay sharp at every scale.
  let frame=null,paintedScale,viewportRect;
  const paint=()=>{frame=null;const {x,y,scale}=camera.state;
    board.style.transform=`translate3d(${x}px,${y}px,0)`;
    if(scale!==paintedScale){board.style.setProperty('--cell',cellSize*scale+'px');board.style.setProperty('--gap',gap*scale+'px');board.dataset.zoom=String(scale);paintedScale=scale;}
  };
  const schedulePaint=()=>{if(frame===null)frame=requestAnimationFrame(paint);};
  const flush=()=>{if(frame!==null)cancelAnimationFrame(frame);paint();};
  const point=event=>{const rect=viewportRect??viewport.getBoundingClientRect();return {x:(event.clientX-rect.left)*viewport.clientWidth/rect.width,y:(event.clientY-rect.top)*viewport.clientHeight/rect.height};};
  const region=()=>{const element=focusArea?.();if(!element)return undefined;const r=element.getBoundingClientRect(),a=viewport.getBoundingClientRect();return {x:(r.left-a.left)*viewport.clientWidth/a.width,y:(r.top-a.top)*viewport.clientHeight/a.height,width:r.width*viewport.clientWidth/a.width,height:r.height*viewport.clientHeight/a.height};};
  const on=(target,type,fn,options)=>{target.addEventListener(type,fn,options);listeners.push(()=>target.removeEventListener(type,fn,options));};
  on(viewport,'pointerdown',event=>{
    if(event.target.closest('button')||(event.pointerType==='mouse'&&event.button!==0))return;
    viewportRect=viewport.getBoundingClientRect();event.preventDefault();camera.down(event.pointerId,point(event));captured.add(event.pointerId);viewport.classList.add('panning');
    try{viewport.setPointerCapture(event.pointerId);}catch{}
  });
  on(viewport,'pointermove',event=>{if(!captured.has(event.pointerId))return;event.preventDefault();camera.move(event.pointerId,point(event));schedulePaint();});
  function release(event){camera.up(event.pointerId);captured.delete(event.pointerId);if(!captured.size){viewport.classList.remove('panning');viewportRect=null;flush();}}
  for(const type of ['pointerup','pointercancel','lostpointercapture'])on(viewport,type,release);
  on(viewport,'wheel',event=>{event.preventDefault();camera.zoom(Math.exp(-clamp(event.deltaY*(event.deltaMode===1?16:1),-300,300)*.003),point(event));schedulePaint();},{passive:false});
  on(viewport,'click',event=>{const control=event.target.closest('[data-zoom]');if(control){camera.zoom(Number(control.dataset.zoom));paint();}});
  const cancel=()=>{camera.cancel();for(const id of captured){try{viewport.releasePointerCapture(id);}catch{}}captured.clear();viewportRect=null;viewport.classList.remove('panning');flush();};
  on(window,'blur',cancel);on(document,'visibilitychange',cancel);
  const initial=dimensions();camera.reveal({x:initial.boardWidth/2,y:initial.boardHeight/2,width:0,height:0},region());paint();
  return {
    cancel,
    resize(){viewportRect=null;camera.resize(dimensions());flush();},
    reveal(elements){
      if(!elements.length)return;flush();const left=Math.min(...elements.map(el=>el.offsetLeft)),top=Math.min(...elements.map(el=>el.offsetTop));
      const scale=camera.state.scale;
      camera.reveal({x:left/scale,y:top/scale,width:(Math.max(...elements.map(el=>el.offsetLeft+el.offsetWidth))-left)/scale,height:(Math.max(...elements.map(el=>el.offsetTop+el.offsetHeight))-top)/scale},region());paint();
    },
    dispose(){cancel();if(frame!==null)cancelAnimationFrame(frame);frame=null;listeners.forEach(remove=>remove());},
  };
}
