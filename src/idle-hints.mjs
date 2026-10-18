/** Attempts restart the idle delay but preserve translation/letters alternation. */
export function createIdleHintClock({now=()=>performance.now(),random=Math.random}={}) {
  let started=now(),next='translation',active=null;
  return {
    reset(){started=now();next='translation';active=null;},
    attempt(){started=now();active=null;},
    tick(words){
      const time=now();
      if(active&&time>=active.until)active=null;
      if(!words.length){active=null;return null;}
      if(active)return active;
      const delay=next==='translation'?30000:60000;
      if(time-started<delay)return null;
      const sample=Number(random());
      const word=words[Math.max(0,Math.min(words.length-1,Math.floor((Number.isFinite(sample)?sample:0)*words.length)))];
      active={type:next,word,until:time+7000};
      next=next==='translation'?'letters':'translation';
      // Translation at 30s, letters at 60s, then alternate every 30s if idle continues.
      started=time-(next==='letters'?30000:0);
      return active;
    },
  };
}
/** Match physical copies of repeated letters without revealing their order. */
export function hintTileIds(word,tiles){
  const remaining=[...word],ids=[];
  for(const tile of tiles){const index=remaining.indexOf(tile.letter);if(index>=0){remaining.splice(index,1);ids.push(tile.id);}}
  return remaining.length?[]:ids;
}
