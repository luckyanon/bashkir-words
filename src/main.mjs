import {createBackgroundMotion} from './background-motion.mjs';
import {loadLexicon} from './dictionary-service.mjs';
import {createGame,HINT_COST} from './game.mjs';
import {buildBoard} from './levels.mjs';
import {createLevelProvider} from './level-generator.mjs';
import {createStorage} from './storage.mjs';
import {createPlatform} from './platform.mjs';
import {bindWheel} from './gestures.mjs';
import {bindBoardCamera} from './board-camera.mjs';
import {ru as t} from './i18n.mjs';
import {coinIcon as coin} from './coin.mjs';
import {layoutOptionsForSave} from './layout-migration.mjs';
import {migrateNounSave} from './noun-migration.mjs';
import {migrateLocalWords} from './local-word-migration.mjs';
import {BACKGROUNDS,availableBackground} from './backgrounds.mjs';
import {renderAttribution} from './attribution.mjs';

const app=document.querySelector('#app'),modal=document.querySelector('#modal');
const licenseModal=document.createElement('dialog');licenseModal.className='license-modal';licenseModal.setAttribute('aria-labelledby','license-title');document.body.append(licenseModal);
let licenseRestoreFocus,licenseContent,boardCamera;
let lexicon,poolData,levels,game,storage,platform,view='menu',wheelInput,boardObserver,toastTimer,restoreFocus,lastCompletionReward=0,uiTiles=[],tilesLevel='',shuffleTimer,completionTimer,giftTimer;
const cellTimers=new Map();
const closingDialogs=new WeakMap();let screenTransition=false;
async function animateExit(element,duration=180){
  if(!element||!motionEnabled())return;
  if(element.matches('.hardcore-screen')){
    const field=element.querySelector('.map-viewport');
    await Promise.all([...element.querySelectorAll('.game-header,.wheel-area,.bottom-actions')].map(child=>animateExit(child,duration)).concat(field?[field.animate([{opacity:getComputedStyle(field).opacity},{opacity:0}],{duration,fill:'forwards'}).finished.catch(()=>{})]:[]));return;
  }
  const current=getComputedStyle(element);const from={opacity:current.opacity,transform:current.transform};element.style.animation='none';
  try{await element.animate([from,{opacity:0,transform:'scale(.94)'}],{duration,easing:'ease-in',fill:'forwards'}).finished;}catch{}
}
function dismissDialog(dialog){
  if(closingDialogs.has(dialog))return closingDialogs.get(dialog);
  if(!dialog.open)return Promise.resolve();
  const task=(async()=>{await animateExit(dialog,158);dialog.close();closingDialogs.delete(dialog);})();
  closingDialogs.set(dialog,task);return task;
}
async function navigateView(next){
  if(screenTransition)return;screenTransition=true;cancelInput();disposeInput();app.inert=true;
  try{await Promise.all([closeModal(true),animateExit(app.firstElementChild)]);view=next;render();}finally{app.inert=false;screenTransition=false;}
}
const icons={menu:'<path d="M4 6h16M4 12h16M4 18h16"/>',back:'<path d="m14 5-7 7 7 7"/>',close:'<path d="m6 6 12 12M18 6 6 18"/>',shuffle:'<path d="M3 7h3c5 0 7 10 12 10h3m-4-3 3 3-3 3M3 17h3c2 0 4-3 6-6s4-4 6-4h3m-4-3 3 3-3 3"/>',bulb:'<path d="M9 18h6m-5 3h4M8 14c-5-5-2-11 4-11s9 6 4 11l-1 2H9z"/>',book:'<path d="M12 5v15M3 4c4-1 6 0 9 2 3-2 5-3 9-2v14c-4-1-6 0-9 2-3-2-5-3-9-2z"/>',trophy:'<path d="M8 3h8v7c0 6-8 6-8 0zM8 5H4v3c0 3 3 4 5 4m7-7h4v3c0 3-3 4-5 4M12 15v5m-4 1h8"/>',lock:'<rect x="5" y="10" width="14" height="11" rx="3"/><path d="M8 10V7a4 4 0 0 1 8 0v3"/>'};
const icon=name=>`<svg viewBox="0 0 24 24" aria-hidden="true">${icons[name]||''}</svg>`;
const escape=value=>String(value??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const button=(action,label,cls='secondary',extra='')=>`<button class="${cls}" data-action="${action}" ${extra}>${label}</button>`;
let toastHideTimer;
function toast(message,duration=2400){
  clearTimeout(toastTimer);clearTimeout(toastHideTimer);
  const el=document.querySelector('#toast');el.textContent=message;
  if(typeof el.showPopover==='function'){if(el.matches(':popover-open'))el.hidePopover();el.showPopover();}
  else {const host=licenseModal.open?licenseModal:modal.open?modal:document.body;host.append(el);}
  el.classList.add('visible');
  toastTimer=setTimeout(()=>{el.classList.remove('visible');toastHideTimer=setTimeout(()=>{if(typeof el.hidePopover==='function'&&el.matches(':popover-open'))el.hidePopover();},200);},duration);
}
function cancelInput(){wheelInput?.cancel();boardCamera?.cancel();}
function disposeInput(){boardCamera?.dispose();boardCamera=null;wheelInput?.dispose();wheelInput=null;boardObserver?.disconnect();boardObserver=null;clearTimeout(shuffleTimer);clearTimeout(completionTimer);completionTimer=null;for(const timer of cellTimers.values())clearTimeout(timer);cellTimers.clear();}
async function closeLicense(){await dismissDialog(licenseModal);const focus=licenseRestoreFocus;licenseRestoreFocus=null;if(focus?.isConnected)focus.focus({preventScroll:true});}
let translationReturn;
async function closeModal(all=false){
  if(all)translationReturn=null;
  else if(translationReturn && modal.open){
    const previous=translationReturn;translationReturn=null;
    await showModal(t.found,`<div class="found-scroll" tabindex="0" role="region" aria-label="${t.found}">${wordList()}</div>`);
    const list=modal.querySelector('.found-scroll');if(list)list.scrollTop=previous.scrollTop;
    [...modal.querySelectorAll('[data-action=translate]')].find(el=>el.dataset.word===previous.word)?.focus({preventScroll:true});return;
  }
if(licenseModal.open){await closeLicense();if(!all)return;}await dismissDialog(modal);const focus=restoreFocus;restoreFocus=null;if(focus?.isConnected)focus.focus({preventScroll:true});else document.querySelector('#found-words,[data-action="continue"]')?.focus({preventScroll:true});}
async function showModal(title,body){translationReturn=null;cancelInput();const focus=modal.open?restoreFocus:document.activeElement;if(modal.open)await dismissDialog(modal);restoreFocus=focus;modal.innerHTML=`<div class="modal-inner"><div class="modal-header">${title?`<h2 id="modal-title">${escape(title)}</h2>`:'<span id="modal-title" class="visually-hidden">Обратная связь</span>'}${button('close',icon('close'),'close-modal',`aria-label="${t.close}"`)}</div>${body}</div>`;modal.style.animation='';modal.getAnimations().forEach(animation=>animation.cancel());modal.showModal();}
modal.addEventListener('click',event=>{if(event.target===modal){const r=modal.getBoundingClientRect();if(event.clientX<r.left||event.clientX>r.right||event.clientY<r.top||event.clientY>r.bottom)closeModal();}});
modal.addEventListener('cancel',event=>{event.preventDefault();closeModal();});
licenseModal.addEventListener('cancel',event=>{event.preventDefault();closeLicense();});
licenseModal.addEventListener('click',event=>{if(event.target===licenseModal){const r=licenseModal.getBoundingClientRect();if(event.clientX<r.left||event.clientX>r.right||event.clientY<r.top||event.clientY>r.bottom)closeLicense();}});
async function showLicense(){
  licenseRestoreFocus=document.activeElement;
  licenseModal.innerHTML=`<div class="modal-inner"><div class="modal-header"><h2 id="license-title">${t.attributionTitle}</h2>${button('license-close',icon('close'),'close-modal',`aria-label="${t.close}"`)}</div><div class="license-content" tabindex="0" role="region" aria-label="${t.attributionTitle}"><p>${t.attributionLoading}</p></div></div>`;
  licenseModal.style.animation='';licenseModal.getAnimations().forEach(animation=>animation.cancel());licenseModal.showModal();
  try{
    if(!licenseContent){const response=await fetch('./ATTRIBUTION.md');if(!response.ok)throw new Error('attribution');licenseContent=renderAttribution(await response.text());}
    if(licenseModal.open)licenseModal.querySelector('.license-content').innerHTML=licenseContent;
  }catch{if(licenseModal.open)licenseModal.querySelector('.license-content').innerHTML=`<p>${t.attributionError}</p>`;}
}
function render(){
  applyBackground();
  disposeInput();clearTimeout(giftTimer);document.body.classList.toggle('no-motion',!game.state.settings.animations);app.setAttribute('aria-busy','false');
  if(view==='menu')renderMenu();else if(view==='victory')renderVictory();else renderGame();
}
function renderMenu(){
  app.innerHTML=`<section class="menu-screen">${button('rating','Рейтинг','menu-rating')}<div class="menu-top"><span class="eyebrow">Башҡортса</span></div><div class="menu-title"><div class="menu-letters" aria-hidden="true"><span>Ә</span><span>Ө</span><span>Ү</span></div><h1>${t.title}</h1><p>${t.subtitle}</p></div><div class="menu-actions">${button('continue',t.continue,'primary')}${button('hardcore','Хардкор','secondary')}${button('gift','','secondary gift-button')}${button('settings',t.settings,'secondary')}${button('about',t.about,'secondary')}</div><p class="menu-note">${t.menuNote}</p>${storage.status.error?`<p class="storage-warning">${t.storageWarning}</p>`:''}</section>`;
  refreshGiftButton();
  const balance=document.createElement('div');balance.className='coin-balance menu-balance';balance.setAttribute('aria-label',`${game.state.coins} ${t.coinsLabel}`);balance.innerHTML=`${coin}<span>${game.state.coins}</span>`;document.querySelector('.menu-screen').prepend(balance);
}
function animateGift(origin){
  const balance=document.querySelector('.menu-balance');if(!balance)return;
  balance.querySelector('span').textContent=game.state.coins;balance.setAttribute('aria-label',`${game.state.coins} ${t.coinsLabel}`);
  if(!motionEnabled()||!origin)return;
  const target=balance.querySelector('.coin').getBoundingClientRect();
  const flight=document.createElement('div');flight.className='gift-flight';flight.setAttribute('aria-hidden','true');flight.innerHTML=coin;flight.style.left=origin.left+origin.width/2-16+'px';flight.style.top=origin.top+origin.height/2-16+'px';document.body.append(flight);
  const dx=target.left+target.width/2-origin.left-origin.width/2,dy=target.top+target.height/2-origin.top-origin.height/2;
  const animation=flight.animate([{transform:'translate(0,0) scale(1)',opacity:1},{transform:`translate(${dx*.2}px,${dy*.35-45}px) scale(1.6) rotate(-15deg)`,offset:.35,opacity:1},{transform:`translate(${dx}px,${dy}px) scale(.7) rotate(12deg)`,opacity:1}],{duration:638,easing:'cubic-bezier(.25,.65,.4,1)'});
  animation.oncancel=()=>flight.remove();animation.onfinish=()=>{flight.remove();if(balance.isConnected&&motionEnabled())balance.animate([{transform:'scale(1)'},{transform:'scale(1.18)'},{transform:'scale(1)'}],{duration:225});};
}
function refreshGiftButton(){
  if(!game||view!=='menu')return;
  const control=document.querySelector('[data-action="gift"]');if(!control)return;
  const {available,remainingMs}=game.getDailyGiftStatus();
  const seconds=Math.ceil(remainingMs/1000),timer=[Math.floor(seconds/3600),Math.floor(seconds/60)%60,seconds%60].map(value=>String(value).padStart(2,'0')).join(':');
  control.disabled=!available;control.setAttribute('aria-label',available?t.giftAvailable:t.giftCountdown(timer));
  control.innerHTML=`<span class="gift-title">${t.gift}</span><span class="gift-reward" aria-hidden="true">${available?coin:timer}</span>`;
  clearTimeout(giftTimer);if(!available)giftTimer=setTimeout(refreshGiftButton,1000);
}
const activeLevelKey=()=>game.state.campaignSeed+':'+(game.isHardcore?'hardcore':'classic')+':'+game.getCurrentLevel().id;
function arrangeTiles(rack){
  const key=activeLevelKey();
  if(tilesLevel!==key||uiTiles.length!==rack.length){uiTiles=rack.map((letter,id)=>({letter,id,slot:id}));tilesLevel=key;return;}
  const slots=new Map();rack.forEach((letter,slot)=>{if(!slots.has(letter))slots.set(letter,[]);slots.get(letter).push(slot);});
  for(const tile of uiTiles)tile.slot=slots.get(tile.letter).shift();
}
function tilePosition(slot,count){const angle=slot/count*Math.PI*2-Math.PI/2;return {left:50+36*Math.cos(angle),top:50+36*Math.sin(angle)};}
function sizeBoard(){
  if(view!=='game')return;
  const area=document.querySelector('.board-area'),board=document.querySelector('.crossword');if(!area||!board)return;
  const level=game.getCurrentLevel(),gap=3;
  const size=game.isHardcore?36:Math.max(12,Math.min(44,Math.floor((area.clientWidth-4-(level.cols-1)*gap)/level.cols),Math.floor((area.clientHeight-16-(level.rows-1)*gap)/level.rows)));
  board.style.setProperty('--cell',size+'px');board.style.setProperty('--gap',gap+'px');
  boardCamera?.resize();
  const wheel=document.querySelector('.wheel');
  if(wheel){const tileSize=Math.min(window.innerHeight<=620?42:53,Math.floor(wheel.clientWidth*.68*Math.sin(Math.PI/uiTiles.length)));wheel.style.setProperty('--letter-size',tileSize+'px');wheel.style.setProperty('--letter-font-size',Math.min(34,Math.floor(tileSize*.65))+'px');}
}
function renderGame(){
  const level=game.getCurrentLevel(),saved=game.getLevelState(),board=buildBoard(level),revealed=new Set(saved.revealed),solved=new Set(saved.solved);
  arrangeTiles(game.rack);
  app.innerHTML=`<section class="game-screen ${game.isHardcore?'hardcore-screen':''}"><header class="game-header"><div class="menu-controls">${button('menu',icon('menu'),'icon-button',`aria-label="${t.menu}"`)}${button('hint',`${icon('bulb')}<span class="hint-price">${HINT_COST}${coin}</span>`,'hint-button compact-hint',`${game.state.coins<HINT_COST||!game.getHiddenCells().length?'disabled':''} aria-label="${t.hint}, ${HINT_COST} ${t.coinsLabel}" title="${t.hint}, ${HINT_COST} ${t.coinsLabel}"`)}</div><div class="level-label"><strong>${game.isHardcore?'Хардкор':`${t.level} ${level.id}`}</strong>${game.isHardcore?`<span class="hardcore-word-count" aria-label="Найдено ${saved.solved.length} из ${level.words.length} слов на поле">${saved.solved.length}/${level.words.length}</span>`:''}</div><div class="game-bank"><div class="coin-balance" aria-label="${game.state.coins} ${t.coinsLabel}">${coin}<span>${game.state.coins}</span></div>${game.isHardcore?button('hardcore-regenerate','<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 7v5h-5M4 17v-5h5M6 7a7 7 0 0 1 12-1l2 3M4 15l2 3a7 7 0 0 0 12-1"/></svg>','hardcore-regenerate','aria-label="Перегенерировать уровень" title="Перегенерировать уровень"'):''}</div></header>${game.isHardcore?'<div class="board-space" aria-hidden="true"></div>':''}<div class="board-area ${game.isHardcore?'map-viewport':''}"><div class="crossword" role="group" aria-label="${t.crossword}" style="grid-template-columns:repeat(${level.cols},var(--cell));grid-template-rows:repeat(${level.rows},var(--cell));--cell:24px">${board.map(cell=>{const shown=revealed.has(cell.key),complete=cell.wordIndexes.some(index=>solved.has(level.words[index].word));return `<div class="cell ${shown?'revealed':''} ${shown&&!complete?'partial':''}" data-cell="${cell.key}" style="grid-row:${cell.row+1};grid-column:${cell.col+1}" aria-label="${shown?escape(cell.letter):t.hiddenCell}">${shown?escape(cell.letter):''}</div>`;}).join('')}</div>${game.isHardcore?'<div class="board-zoom" aria-label="Масштаб поля"><button type="button" data-zoom="1.25" aria-label="Приблизить поле">+</button><button type="button" data-zoom="0.8" aria-label="Отдалить поле">−</button></div>':''}</div><div class="wheel-area"><div id="word-preview" role="status" aria-live="polite">· · ·</div><span id="keyboard-help" class="visually-hidden">${t.keyboardHelp}</span><div class="wheel" aria-label="${t.letterWheel}" aria-describedby="keyboard-help"><svg viewBox="0 0 100 100" aria-hidden="true"><polyline points=""/></svg>${uiTiles.map(tile=>{const p=tilePosition(tile.slot,uiTiles.length);return `<button class="letter" data-tile="${tile.id}" data-slot="${tile.slot}" style="left:${p.left}%;top:${p.top}%" aria-label="${escape(tile.letter)}; ${t.tileLabel} ${tile.id+1}" aria-pressed="false">${escape(tile.letter)}</button>`;}).join('')}${button('shuffle',icon('shuffle'),'wheel-center',`aria-label="${t.shuffle}" title="${t.shuffle}"`)}</div></div><footer class="bottom-actions">${button('found',`${icon('book')}<span>${t.found}</span>`,'found-button','id="found-words"')}</footer></section>`;
  wheelInput=bindWheel(document.querySelector('.wheel'),{tiles:uiTiles,onSubmit:submitWord,onSelect:()=>platform.sound('select'),onPreview:renderPreview});
  sizeBoard();if(game.isHardcore)boardCamera=bindBoardCamera(document.querySelector('.board-area'),document.querySelector('.crossword'),{overview:true,rows:level.rows,cols:level.cols,focusArea:()=>document.querySelector('.board-space')});boardObserver=new ResizeObserver(sizeBoard);boardObserver.observe(document.querySelector('.board-area'));if(game.isHardcore)boardObserver.observe(document.querySelector('.board-space'));
}
let invalidPreviewTimer;
function renderPreview(word){
  clearTimeout(invalidPreviewTimer);
  const preview=document.querySelector('#word-preview');if(!preview)return;preview.classList.remove('invalid-word');
  if(!preview.dataset.cells){preview.replaceChildren();preview.dataset.cells='true';}
  const letters=[...word];while(preview.children.length>letters.length)preview.lastElementChild.remove();
  letters.forEach((letter,index)=>{let cell=preview.children[index];if(!cell){cell=document.createElement('span');cell.className='preview-cell';preview.append(cell);}cell.textContent=letter;});
  preview.classList.toggle('has-word',!!word);preview.setAttribute('aria-label',word||'');
  preview.style.setProperty('--preview-size',Math.min(36,Math.floor((document.querySelector('.wheel-area').clientWidth-Math.max(0,letters.length-1)*4)/Math.max(1,letters.length)))+'px');
}
function wordList(withTranslations=true){
  const saved=game.getLevelState();
  const items=[...saved.solved.map(word=>({word,type:t.target})),...saved.bonus.map(word=>({word,type:t.bonus}))];
  return items.length?`<ul class="word-list">${items.map(item=>`<li class="word-row"><div><strong>${escape(item.word)}</strong><small>${item.type}</small></div>${withTranslations?button('translate',t.translation,'',`data-word="${escape(item.word)}" aria-label="${t.translation}: ${escape(item.word)}"`):''}</li>`).join('')}</ul>`:`<p>${t.noWords}</p>`;
}
function renderVictory(){
  const level=game.getCurrentLevel();
  app.innerHTML=`<section class="victory-screen"><div class="victory-heading"><div class="victory-badge">${icon('trophy')}</div><span class="eyebrow">${game.isHardcore?'Хардкор':`${t.level} ${level.id}`}</span><h1>${t.victory}</h1><p class="muted">${t.victoryNote}</p><div class="reward-chip">${coin}<span>${lastCompletionReward?`${t.reward}: +${lastCompletionReward}`:t.rewardAlready}</span></div></div><div class="victory-words" tabindex="0" role="region" aria-label="${t.found}">${wordList()}</div><div class="victory-actions">${button('next',t.next,'primary')}${button('menu',t.menu,'secondary')}</div></section>`;
}
function showTranslation(word){
  const state=game.getLevelState();if(!state.solved.includes(word)&&!state.bonus.includes(word))return;
  const entry=lexicon.getEntry(word),meanings=lexicon.getMeanings(word);
  const positions=(entry?.partsOfSpeech||[]).map(pos=>t.partsOfSpeech[typeof pos==='string'?pos:pos.code]||'').filter(Boolean);
  const links=(entry?.sources||[]).map(source=>source.url||source.articleUrl||source.pageUrl).filter(url=>typeof url==='string'&&url.startsWith('https://'));
  return showModal(t.translationTitle,`<div class="translation-word">${escape(word)}</div>${positions.length?`<span class="translation-pos">${escape([...new Set(positions)].join(', '))}</span>`:''}${meanings.length?`<div class="meaning-scroll" tabindex="0" role="region" aria-label="${t.meaningsScroll}"><ol class="meaning-list">${meanings.map(meaning=>`<li>${escape(meaning.text)}</li>`).join('')}</ol></div>`:`<p>${t.noTranslation}</p>`}${entry?`<details class="translation-source"><summary>${t.sourcesLabel}</summary><p>${t.localMeaning}<br>${t.entryLabel}: ${escape(entry.id)}</p>${[...new Set(links)].slice(0,3).map((url,index)=>`<a href="${escape(url)}" target="_blank" rel="noopener noreferrer">${t.sourceLabel} ${index+1}</a>`).join('')}</details>`:''}`);
}
function motionEnabled(){return game.state.settings.animations&&!window.matchMedia('(prefers-reduced-motion: reduce)').matches;}
function refreshGameBoard(result){
  const level=game.getCurrentLevel(),saved=game.getLevelState(),revealed=new Set(saved.revealed),solved=new Set(saved.solved),newWords=new Set(result.newlySolved||[]),animated=[];
  for(const cell of buildBoard(level)){
    const el=document.querySelector(`[data-cell="${cell.key}"]`);if(!el)continue;
    const shown=revealed.has(cell.key),complete=cell.wordIndexes.some(index=>solved.has(level.words[index].word));
    el.textContent=shown?cell.letter:'';el.setAttribute('aria-label',shown?cell.letter:t.hiddenCell);
    el.classList.toggle('revealed',shown);el.classList.toggle('partial',shown&&!complete);
    const found=cell.wordIndexes.some(index=>newWords.has(level.words[index].word));
    if(found||cell.key===result.key){clearTimeout(cellTimers.get(el));el.classList.remove('word-found','hint-reveal');animated.push({el,found});}
  }
  const counter=document.querySelector('.hardcore-word-count');
  if(counter){counter.textContent=`${saved.solved.length}/${level.words.length}`;counter.setAttribute('aria-label',`Найдено ${saved.solved.length} из ${level.words.length} слов на поле`);}
  const balance=document.querySelector('.coin-balance');
  if(balance){balance.querySelector('span').textContent=game.state.coins;balance.setAttribute('aria-label',`${game.state.coins} ${t.coinsLabel}`);}
  const hint=document.querySelector('[data-action="hint"]');if(hint)hint.disabled=game.state.coins<HINT_COST||!game.getHiddenCells().length;
  // Restart only the cells involved; the wheel and previous reveals stay in place.
  if(animated.length)boardCamera?.reveal(animated.map(item=>item.el));
  if(animated.length){void document.querySelector('.crossword').offsetWidth;for(const {el,found} of animated){el.classList.add(found?'word-found':'hint-reveal');cellTimers.set(el,setTimeout(()=>{el.classList.remove('word-found','hint-reveal');cellTimers.delete(el);},found?690:195));}}
}
function finishOnBoard(result){
  refreshGameBoard(result);if(!result.completed)return;
  lastCompletionReward=Math.max(lastCompletionReward,result.completionReward||0);if(completionTimer)return;platform.vibrate(35);
  const id=activeLevelKey();
  clearTimeout(completionTimer);completionTimer=setTimeout(()=>{if(view!=='game'||activeLevelKey()!==id)return;navigateView('victory');},motionEnabled()?750:263);
}
function shakeFoundWord(word){
  const level=game.getCurrentLevel(),indexes=new Set(level.words.map((p,i)=>p.word===word?i:-1));
  const cells=buildBoard(level).filter(cell=>cell.wordIndexes.some(i=>indexes.has(i)));
  for(const cell of cells){const el=document.querySelector(`[data-cell="${cell.key}"]`);if(!el)continue;clearTimeout(cellTimers.get(el));el.classList.remove('word-found','hint-reveal','word-repeat');void el.offsetWidth;el.classList.add('word-repeat');cellTimers.set(el,setTimeout(()=>{el.classList.remove('word-repeat');cellTimers.delete(el);},400));}
}
function rejectWord(word){
  renderPreview(word);
  const preview=document.querySelector('#word-preview');if(!preview)return;
  void preview.offsetWidth;preview.classList.add('invalid-word');
  platform.sound('invalid');
  invalidPreviewTimer=setTimeout(()=>{if(preview.isConnected)renderPreview('');},420);
}
function submitWord(word){
  const result=game.submit(word);if(result.type==='repeat'){if(result.bonus){toast(`${t.bonus}: ${result.word}`);platform.sound('bonus');}else{shakeFoundWord(result.word);platform.sound('repeat');}return;}if(result.type==='excluded-length'){toast(t.excludedWordLength(result.length),1800);rejectWord(word);return;}if(!['target','bonus'].includes(result.type)){rejectWord(word);return;}
  if(result.type==='bonus')toast(`${t.bonus}: ${result.word}`);
  platform.sound(result.completed?'victory':result.type);
  finishOnBoard(result);
}
async function start(id,replay=false){if(screenTransition||!(replay?game.replay(id):game.startLevel(id)))return;lastCompletionReward=0;await navigateView(game.getLevelState(id).completed?'victory':'game');}
async function regenerateHardcore(){
  if(screenTransition||!game.isHardcore)return;screenTransition=true;cancelInput();disposeInput();app.inert=true;
  try{await Promise.all([closeModal(true),animateExit(app.firstElementChild)]);if(game.regenerateHardcore()){lastCompletionReward=0;view='game';render();}}finally{app.inert=false;screenTransition=false;}
}
function animateShuffle(){
  const wheel=document.querySelector('.wheel');if(!wheel)return;
  clearTimeout(shuffleTimer);cancelInput();arrangeTiles(game.shuffle());wheel.classList.add('is-shuffling');
  for(const tile of uiTiles){const el=wheel.querySelector(`[data-tile="${tile.id}"]`),p=tilePosition(tile.slot,uiTiles.length);el.style.left=p.left+'%';el.style.top=p.top+'%';el.dataset.slot=tile.slot;}
  wheelInput?.refresh();
  shuffleTimer=setTimeout(()=>{wheel.classList.remove('is-shuffling');wheelInput?.refresh();},motionEnabled()?405:0);
}
function campaignSeed(){return String(Date.now())+'-'+crypto.getRandomValues(new Uint32Array(1))[0];}
function installGame(provider,saved=null){levels=provider;game=createGame(provider,lexicon,saved,{hardcoreSource:createLevelProvider(poolData,lexicon,{seed:provider.seed+':hardcore',hardcore:true}),onChange:state=>{storage.save(state).then(ok=>{if(!ok)toast(t.storageWarning);});}});}
async function action(name,element){
  if(name==='gift'){const origin=element.querySelector('.coin')?.getBoundingClientRect();const result=game.claimDailyGift();refreshGiftButton();if(result.type==='gift'){animateGift(origin);platform.sound('bonus');toast(t.giftReceived);}else toast(t.giftClaimed);return;}
  if(name==='license-close'){await closeLicense();return;}
  if(name==='attribution'){await showLicense();return;}
  if(name==='close'){await closeModal();return;}
  if(name==='menu'){await navigateView('menu');return;}
  if(name==='continue'){await start(game.state.currentLevel);return;}
  if(name==='hardcore'){if(screenTransition||!game.setMode('hardcore'))return;lastCompletionReward=0;await navigateView(game.getLevelState().completed?'victory':'game');return;}
  if(name==='hardcore-regenerate'){if(screenTransition||!game.isHardcore||view!=='game')return;await showModal('Пересобрать уровень?',`<div class="modal-buttons">${button('close',t.cancel,'secondary')}${button('confirm-hardcore-regenerate','Пересобрать','primary purple')}</div>`);return;}
  if(name==='confirm-hardcore-regenerate'){if(modal.open&&element.closest('#modal'))await regenerateHardcore();return;}
  if(name==='next'){if(game.isHardcore){await regenerateHardcore();return;}await start(game.getNextLevelId());return;}
  if(name==='shuffle'){animateShuffle();return;}
  if(name==='hint'){cancelInput();const result=game.hint();if(result.type==='insufficient')toast(t.insufficient);else if(result.type==='hint'){platform.sound(result.completed?'victory':'hint');finishOnBoard(result);}return;}
  if(name==='found'){await showModal(t.found,`<div class="found-scroll" tabindex="0" role="region" aria-label="${t.found}">${wordList()}</div>`);return;}
  if(name==='translate'){const list=modal.open?modal.querySelector('.found-scroll'):null;const previous=list?{scrollTop:list.scrollTop,word:element.dataset.word}:null;await showTranslation(element.dataset.word);translationReturn=previous;return;}
  if(name==='settings'){await showSettings();return;}
  if(name==='dictionary-feedback'){await showModal('',`<form id="dictionary-feedback-form"><label class="feedback-label" for="dictionary-feedback-text">Каких слов не хватает?</label><textarea id="dictionary-feedback-text" name="message" rows="5" placeholder="Напишите слово и его перевод" required></textarea><p class="dialog-note">Отправка пока недоступна.</p><button type="submit" class="primary purple feedback-submit">Отправить</button></form>`);return;}
  if(name==='reset-progress'){await showModal('Сбросить прогресс?',`<p>Игра начнётся с первого уровня и 90 монет. Найденные слова, полученные награды и открытые фоны будут сброшены. Отменить сброс нельзя.</p><div class="modal-buttons">${button('settings','Отмена','secondary')}${button('confirm-reset-progress','Сбросить','reset-progress-button')}</div>`);return;}
  if(name==='confirm-reset-progress'){element.disabled=true;clearTimeout(completionTimer);clearTimeout(shuffleTimer);clearTimeout(giftTimer);game.reset();await closeModal(true);await navigateView('menu');toast('Прогресс сброшен');return;}
  if(name==='background'){
    const bg=BACKGROUNDS.find(bg=>bg.id===element.dataset.background);if(!bg)return;
    if(game.ownsBackground(bg.id)){game.setSettings({background:bg.id});applyBackground();updateBackgroundCards();return;}
    await showModal('Купить фон?',`<div class="background-purchase"><img src="${bg.file}" alt=""><strong>${bg.name}</strong><span class="background-price" aria-label="${bg.price} монет">${bg.price}${coin}</span></div>${game.state.coins<bg.price?'<p class="dialog-note">Недостаточно монет.</p>':''}<div class="modal-buttons">${button('settings','Отмена','secondary')}${button('confirm-background-purchase','Купить','primary purple',`data-background="${bg.id}" ${game.state.coins<bg.price?'disabled':''}`)}</div>`);return;
  }
  if(name==='confirm-background-purchase'){
    if(!modal.open||!element.closest('#modal'))return;element.disabled=true;
    const result=game.purchaseBackground(element.dataset.background);
    if(result.type==='insufficient'){toast('Недостаточно монет.');element.disabled=false;return;}
    if(result.type!=='purchased'&&result.type!=='owned')return;
    applyBackground();refreshCoinBalances();await showSettings();return;
  }
  if(name==='toggle'){const setting=element.dataset.setting;game.setSettings({[setting]:!(setting==='backgroundMotion'?game.state.settings[setting]!==false:game.state.settings[setting])});document.body.classList.toggle('no-motion',!game.state.settings.animations);element.setAttribute('aria-checked',String(game.state.settings[setting]));backgroundMotion?.setEnabled(game.state.settings.backgroundMotion!==false&&game.state.settings.animations);return;}
  if(name==='rating'){await showModal('Рейтинг','<p class="rating-placeholder">В разработке</p>');return;}
  if(name==='about'){await showModal(t.about,`<div class="about-stats"><div><strong>${lexicon.dictionary.size.toLocaleString('ru')}</strong><span>${t.dictionaryWords}</span></div><div><strong>∞</strong><span>${t.endless}</span></div></div><p>${t.aboutLicense}</p>${button('attribution',t.attribution,'attribution-button')}<p class="dialog-note">${t.aboutReview}<br><button class="dictionary-feedback-link" data-action="dictionary-feedback">В словарной базе могут отсутствовать некоторые слова. Нашли такие? Сообщите нам.</button></p><div class="dictionary-contacts"><p class="dialog-note">Задать вопрос:</p><div class="dictionary-contact-links"><a href="https://t.me/goinmasss" target="_blank" rel="noopener noreferrer"><img src="./assets/contact-telegram.webp" alt="">Telegram</a><a href="https://vk.ru/luckyanon" target="_blank" rel="noopener noreferrer"><img src="./assets/contact-vk.png" alt="">VK</a></div></div>`);return;}
}
let backgroundFrameImage,backgroundMotion;
const backgroundLayer=document.createElement('div');backgroundLayer.className='background-layer';backgroundLayer.setAttribute('aria-hidden','true');document.body.prepend(backgroundLayer);
const backgroundFrameImages=new Map();
function positionBackground(){
  const image=backgroundFrameImage;if(!image?.naturalWidth)return;
  const width=window.innerWidth+48,height=window.innerHeight+48;
  const scale=Math.max(width/image.naturalWidth,height/image.naturalHeight);
  const renderedWidth=image.naturalWidth*scale;
  // Place the point at 2/3 of the image in the viewport centre; keep the edges covered.
  const left=Math.max(width-renderedWidth,Math.min(0,width/2-renderedWidth*(2/3)));
  backgroundLayer.style.backgroundPositionX=`${left}px`;
}
function applyBackground(){
  const bg=availableBackground(game.state.settings.background,game.state.backgrounds.owned)||BACKGROUNDS[0];
  backgroundLayer.style.backgroundImage=`url("${bg.file}")`;document.body.style.backgroundImage='none';backgroundMotion?.setEnabled(game.state.settings.backgroundMotion!==false&&game.state.settings.animations);
  let image=backgroundFrameImages.get(bg.id);
  if(!image){image=new Image();image.onload=()=>{if(backgroundFrameImage===image)positionBackground();};image.src=bg.file;backgroundFrameImages.set(bg.id,image);}
  backgroundFrameImage=image;positionBackground();
}
window.addEventListener('resize',positionBackground);
function refreshCoinBalances(){
  for(const balance of document.querySelectorAll('.coin-balance')){balance.querySelector('span').textContent=game.state.coins;balance.setAttribute('aria-label',`${game.state.coins} ${t.coinsLabel}`);}
  const hint=document.querySelector('[data-action=hint]');if(hint)hint.disabled=game.state.coins<HINT_COST||!game.getHiddenCells().length;
}
function updateBackgroundCards(){for(const card of modal.querySelectorAll('.background-card')){const selected=card.dataset.background===(game.state.settings.background||'ufa');card.setAttribute('aria-pressed',String(selected));}}
function showSettings(){return showModal(t.settings,`<div>${[['sound',t.sound],['animations',t.animations],['backgroundMotion','Движение фона']].map(([key,label])=>`<div class="settings-row"><span id="label-${key}">${label}</span><button class="switch" role="switch" aria-checked="${key==='backgroundMotion'?game.state.settings[key]!==false:game.state.settings[key]}" aria-labelledby="label-${key}" data-action="toggle" data-setting="${key}"></button></div>`).join('')}</div><div class="background-picker"><div class="background-gallery-header"><h3>Фон игры</h3></div><div class="background-gallery" role="group" aria-label="Галерея фонов">${BACKGROUNDS.map(bg=>`<button class="background-card" data-action="background" data-background="${bg.id}" aria-pressed="${(game.state.settings.background||'ufa')===bg.id}" ><img src="${bg.file}" alt=""><strong>${bg.name}</strong>${game.ownsBackground(bg.id)?'':`<small class="background-price" aria-label="${bg.price} монет">${bg.price}${coin}</small>`}</button>`).join('')}</div></div><p class="dialog-note">${t.settingsNote}</p><div class="reset-progress-section">${button('reset-progress','Сбросить прогресс','reset-progress-button')}</div>`);}
document.addEventListener('click',event=>{const el=event.target.closest('[data-action]');if(el&&!el.disabled)action(el.dataset.action,el).catch(error=>{console.error(error);toast(t.actionError);});});
window.addEventListener('keydown',event=>{if(!event.defaultPrevented&&event.key==='Escape'&&!modal.open&&!licenseModal.open&&view==='game')navigateView('menu');});
window.addEventListener('resize',sizeBoard);
window.addEventListener('focus',refreshGiftButton);
document.addEventListener('visibilitychange',()=>{if(!document.hidden)refreshGiftButton();});
async function boot(){
  try{
    const plugins=window.BashkirNativePlugins||window.Capacitor?.Plugins||{};
    platform=createPlatform({onCancel:cancelInput,onBack:()=>{if(modal.open||licenseModal.open)closeModal();else if(view!=='menu')navigateView('menu');else plugins.App?.exitApp?.();},settings:()=>game?.state.settings||{sound:true},plugins});
    storage=createStorage(platform.storageAdapter);backgroundMotion=createBackgroundMotion(backgroundLayer,{native:plugins.BackgroundMotion,app:plugins.App});
    const [loadedLexicon,pools,saved]=await Promise.all([loadLexicon('./',globalThis.fetch),fetch('./data/level-pools.json').then(r=>{if(!r.ok)throw new Error('level pools');return r.json();}),storage.load()]);
    lexicon=loadedLexicon;poolData=pools;
    const seed=saved?.campaignSeed||campaignSeed();
    const provider=createLevelProvider(pools,lexicon,{seed,...layoutOptionsForSave(saved,pools.generatorVersion)});
    const migrated=migrateNounSave(saved,provider);
    const wordsMigrated=migrateLocalWords(migrated.saved,createLevelProvider(pools,lexicon,{seed,...layoutOptionsForSave(saved,pools.generatorVersion),includeLocalWords:!!saved?.wordRevision,applyLevelDifficulty:!!saved?.difficultyRevision}),provider);
    installGame(provider,wordsMigrated.saved);
    if(migrated.changed || wordsMigrated.changed || saved && saved.layoutVersion!==2)await storage.save(game.snapshot());
    render();if(saved?.version===1){await storage.save(game.snapshot());toast(t.progressUpdated);}else if([2,3].includes(saved?.generatorVersion))await storage.save(game.snapshot());
    if(!window.Capacitor?.isNativePlatform?.()&&'serviceWorker' in navigator&&location.protocol!=='file:')navigator.serviceWorker.register('./sw.js').catch(()=>{});
  }catch(error){console.error(error);app.setAttribute('aria-busy','false');app.innerHTML=`<div class="loading"><h1>${t.title}</h1><p>${t.loadError}</p><p class="menu-note">${t.loadErrorHelp}</p><button class="primary" onclick="location.reload()">${t.retry}</button></div>`;}
}
boot();








document.addEventListener('submit',event=>{if(event.target.id!=='dictionary-feedback-form')return;event.preventDefault();toast('Отправка пока недоступна. Текст не отправлен.');});













