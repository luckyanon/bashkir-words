/** Original, public-domain (CC0) pentatonic ambient piece with a soft kurai-inspired timbre.
 * Not a recording or transcription of an existing song. Independent of sound effects. */
export function createMusic({AudioContext=globalThis.AudioContext||globalThis.webkitAudioContext,win=globalThis.window,doc=globalThis.document}={}){
  let context,enabled=true,unlocked=false,foreground=true,timer,master,cursor=0,nextTime=0,starting=false;
  const voices=new Set(),beat=60/68;
  // D minor pentatonic; four slow call-and-response phrases with spacious rests.
  const score=[[74,2],[77,1],[79,1],[81,3],[0,1],[79,2],[77,2],[74,3],[0,1],
    [72,2],[74,1],[77,1],[79,3],[0,1],[77,2],[74,2],[72,3],[0,1],
    [69,2],[72,2],[74,3],[0,1],[77,2],[79,1],[77,1],[74,3],[0,1],
    [72,2],[69,2],[67,2],[69,2],[74,4],[0,4]];
  function voice(note,time,duration){
    if(!note)return;
    const gain=context.createGain(),fundamental=context.createOscillator(),overtone=context.createOscillator(),soft=context.createGain();
    const frequency=440*2**((note-69)/12);
    fundamental.type='sine';fundamental.frequency.value=frequency;
    overtone.type='sine';overtone.frequency.value=frequency*2;soft.gain.value=.12;
    fundamental.connect(gain);overtone.connect(soft);soft.connect(gain);gain.connect(master);
    gain.gain.setValueAtTime(0,time);gain.gain.linearRampToValueAtTime(.16,time+.15);
    gain.gain.exponentialRampToValueAtTime(.06,time+Math.max(.2,duration*.65));gain.gain.linearRampToValueAtTime(0,time+duration+.35);
    for(const node of [fundamental,overtone]){voices.add(node);node.start(time);node.stop(time+duration+.4);node.onended=()=>{voices.delete(node);node.disconnect();};}
    overtone.onended=()=>{voices.delete(overtone);overtone.disconnect();soft.disconnect();gain.disconnect();};
  }
  function schedule(){
    if(!enabled||!foreground||!unlocked||doc?.hidden)return;
    while(nextTime<context.currentTime+1.5){const [note,beats]=score[cursor];voice(note,nextTime,beats*beat*.9);if(cursor%8===0)voice(50,nextTime,7*beat);nextTime+=beats*beat;cursor=(cursor+1)%score.length;}
  }
  function stop(){clearInterval(timer);timer=null;for(const voice of voices)try{voice.stop();}catch{}voices.clear();context?.suspend().catch(()=>{});}
  async function start(){
    if(!enabled||!foreground||!unlocked||doc?.hidden||!AudioContext||timer||starting)return;
    starting=true;
    try{context||=new AudioContext();if(!master){master=context.createGain();master.gain.value=.22;master.connect(context.destination);}await context.resume();if(!enabled||!foreground||doc?.hidden)return;nextTime=context.currentTime+.1;schedule();timer=setInterval(schedule,500);}catch{}finally{starting=false;}
  }
  const unlock=()=>{unlocked=true;start();};
  const visibility=()=>{if(doc.hidden)stop();else start();};
  win?.addEventListener('pointerdown',unlock);win?.addEventListener('keydown',unlock);doc?.addEventListener('visibilitychange',visibility);
  return {setEnabled(value){enabled=!!value;if(enabled)start();else stop();},setActive(value){foreground=!!value;if(foreground)start();else stop();},dispose(){stop();context?.close().catch(()=>{});win?.removeEventListener('pointerdown',unlock);win?.removeEventListener('keydown',unlock);doc?.removeEventListener('visibilitychange',visibility);}};
}
