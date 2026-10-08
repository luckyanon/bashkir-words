import {getWordCells} from './levels.mjs';

// Incremental placement keeps large fields practical without dropping any words.
export function createHardcoreLayout(source,random){
 const pending=source.map(entry=>({...entry,tie:random()})).sort((a,b)=>b.word.length-a.word.length||a.tie-b.tie);
 const board=new Map(),byLetter=new Map(),words=[];
 let top=0,left=0,bottom=-1,right=-1;
 function add(placement){
  const {tie,...word}=placement;words.push(word);
  for(const cell of getWordCells(word)){
   const previous=board.get(cell.key);
   if(previous)previous.directions.add(word.direction);
   else {const item={...cell,directions:new Set([word.direction])};board.set(cell.key,item);if(!byLetter.has(cell.letter))byLetter.set(cell.letter,[]);byLetter.get(cell.letter).push(item);}
   top=Math.min(top,cell.row);left=Math.min(left,cell.col);bottom=Math.max(bottom,cell.row);right=Math.max(right,cell.col);
  }
 }
 function score(placement){
  const dr=placement.direction==='down'?1:0,dc=1-dr,cells=getWordCells(placement),first=cells[0],last=cells.at(-1);
  if(board.has(`${first.row-dr},${first.col-dc}`)||board.has(`${last.row+dr},${last.col+dc}`))return null;
  let crossings=0;
  for(const cell of cells){const previous=board.get(cell.key);
   if(previous){if(previous.letter!==cell.letter||previous.directions.has(placement.direction))return null;crossings++;}
   else if(board.has(`${cell.row-dc},${cell.col-dr}`)||board.has(`${cell.row+dc},${cell.col+dr}`))return null;
  }
  if(!crossings)return null;
  const rows=Math.max(bottom,last.row)-Math.min(top,first.row)+1,cols=Math.max(right,last.col)-Math.min(left,first.col)+1;
  return rows*cols+Math.max(rows,cols)**2*.4-crossings*18+random();
 }
 function connect(entry){
  let best,bestScore=Infinity;const seen=new Set();
  for(const [offset,letter] of [...entry.word].entries())for(const cell of byLetter.get(letter)||[])for(const direction of ['across','down']){
   if(cell.directions.has(direction))continue;
   const row=cell.row-(direction==='down'?offset:0),col=cell.col-(direction==='across'?offset:0),key=`${row},${col},${direction}`;
   if(seen.has(key))continue;seen.add(key);
   const candidate={...entry,row,col,direction},value=score(candidate);
   if(value!==null&&value<bestScore){best=candidate;bestScore=value;}
  }
  if(!best)return false;add(best);return true;
 }
 if(!pending.length)throw new TypeError('Hardcore field needs target words');
 add({...pending.shift(),row:0,col:0,direction:random()<.5?'across':'down'});
 while(pending.length){
  let connected=false;
  for(let i=0;i<pending.length;){if(connect(pending[i])){pending.splice(i,1);connected=true;}else i++;}
  if(connected)continue;
  const entry=pending.shift(),rows=bottom-top+1,cols=right-left+1;
  // A clear one-cell gap prevents accidental undeclared word chains.
  if(cols<=rows)add({...entry,row:top,col:right+2,direction:'down'});
  else add({...entry,row:bottom+2,col:left,direction:'across'});
 }
 return {rows:bottom-top+1,cols:right-left+1,words:words.map(word=>({...word,row:word.row-top,col:word.col-left}))};
}
