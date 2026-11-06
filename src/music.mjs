/** Original CC0 melody and synthesized wind instrument; no sampled recording. */
export const MUSIC_BEAT = 60 / 66;
// D-major pentatonic, with a high answering phrase and room to breathe.
// [MIDI note (0 = rest), beats, optional short grace note]
export const KURAI_SCORE = Object.freeze([
  [74,2.5],[76,.5],[78,1],[81,2.5,78],[78,.5],[76,1],[74,3],[0,1.5],
  [71,1],[74,2,71],[76,1],[78,2],[76,1],[74,1],[71,2],[69,2.5],[0,1.5],
  [74,1.5],[78,.5],[81,2],[83,1],[86,3,83],[83,1],[81,2],[78,2.5],[0,1.5],
  [81,1],[78,1],[76,2,74],[74,1],[71,1],[69,2],[71,1],[74,4],[0,3],
].map(Object.freeze));

/** Shared by live playback and OfflineAudioContext for an exact audio preview. */
export function createKuraiInstrument(context, destination) {
  const voices = new Set();
  const wave = context.createPeriodicWave(new Float32Array(7),
    new Float32Array([0,1,.32,.12,.045,.025,.012]));
  // One reusable breath buffer, filtered per note; no work in a JS audio callback.
  const breath = context.createBuffer(1, Math.ceil(context.sampleRate * 2), context.sampleRate);
  const samples = breath.getChannelData(0);
  let seed = 9173;
  for (let i=0;i<samples.length;i++) {
    seed=(Math.imul(seed,1664525)+1013904223)>>>0;
    samples[i]=(seed/4294967296)*2-1;
  }
  function play(note, time, duration, grace=0) {
    if (!note) return;
    const tone=context.createOscillator(),air=context.createBufferSource(),vibrato=context.createOscillator();
    const envelope=context.createGain(),airGain=context.createGain(),depth=context.createGain(),filter=context.createBiquadFilter();
    const frequency=midi=>440*2**((midi-69)/12);
    const end=time+duration,attack=Math.min(.1,duration*.22),release=Math.min(.23,duration*.3);
    tone.setPeriodicWave(wave);
    tone.frequency.setValueAtTime(frequency(grace||note),time);
    if(grace)tone.frequency.setValueAtTime(frequency(note),time+.075);
    tone.detune.setValueAtTime(-11,time);
    tone.detune.linearRampToValueAtTime(1.5,time+attack);
    tone.detune.linearRampToValueAtTime(-5,end);
    // Late, restrained vibrato rather than a constant electronic wobble.
    vibrato.frequency.value=4.7+(note%3)*.13;
    depth.gain.setValueAtTime(0,time);
    depth.gain.setValueAtTime(0,time+Math.min(.3,duration*.35));
    depth.gain.linearRampToValueAtTime(duration>.8?7:2,time+duration*.65);
    vibrato.connect(depth);depth.connect(tone.detune);
    air.buffer=breath;air.loop=true;
    filter.type='bandpass';filter.frequency.value=1400+frequency(note)*.35;filter.Q.value=.65;
    airGain.gain.value=.075;
    air.connect(filter);filter.connect(airGain);airGain.connect(envelope);tone.connect(envelope);
    envelope.gain.setValueAtTime(0,time);
    envelope.gain.linearRampToValueAtTime(.19,time+attack);
    envelope.gain.linearRampToValueAtTime(.15,time+duration*.55);
    envelope.gain.setValueAtTime(.14,end-release);
    envelope.gain.linearRampToValueAtTime(0,end);
    envelope.connect(destination);
    const sources=[tone,air,vibrato],nodes=[...sources,envelope,airGain,depth,filter];
    let remaining=sources.length,cleaned=false;
    const voice={stop(){for(const source of sources)try{source.stop();}catch{}cleanup();}};
    function cleanup(){if(cleaned)return;cleaned=true;for(const node of nodes)node.disconnect();voices.delete(voice);}
    for(const source of sources){source.onended=()=>{if(--remaining===0)cleanup();};source.start(time);source.stop(end+.02);}
    voices.add(voice);
  }
  return {play,stop(){for(const voice of [...voices])voice.stop();}};
}

export function createMusic({AudioContext=globalThis.AudioContext||globalThis.webkitAudioContext,win=globalThis.window,doc=globalThis.document}={}){
  let context,enabled=true,unlocked=false,foreground=true,timer,master,instrument,cursor=0,nextTime=0,starting=false,disposed=false;
  const canPlay=()=>!disposed&&enabled&&foreground&&unlocked&&!doc?.hidden;
  function schedule(){
    if(!canPlay())return;
    // Do not replay an entire backlog after a throttled browser timer.
    if(nextTime<context.currentTime-.2)nextTime=context.currentTime+.05;
    while(nextTime<context.currentTime+1.2){
      const [note,beats,grace]=KURAI_SCORE[cursor];
      instrument.play(note,nextTime,beats*MUSIC_BEAT*.93,grace);
      nextTime+=beats*MUSIC_BEAT;cursor=(cursor+1)%KURAI_SCORE.length;
    }
  }
  function stop(){clearInterval(timer);timer=null;instrument?.stop();context?.suspend().catch(()=>{});}
  async function start(){
    if(!canPlay()||!AudioContext||timer||starting)return;
    starting=true;
    try{
      context||=new AudioContext();
      if(!master){master=context.createGain();master.gain.value=.25;master.connect(context.destination);instrument=createKuraiInstrument(context,master);}
      await context.resume();
      if(!canPlay())return;
      nextTime=context.currentTime+.1;schedule();timer=setInterval(schedule,250);
    }catch{}finally{starting=false;}
  }
  const unlock=()=>{unlocked=true;start();};
  const visibility=()=>{if(doc.hidden)stop();else start();};
  win?.addEventListener('pointerdown',unlock);win?.addEventListener('keydown',unlock);doc?.addEventListener('visibilitychange',visibility);
  return {
    setEnabled(value){enabled=!!value;if(enabled)start();else stop();},
    setActive(value){foreground=!!value;if(foreground)start();else stop();},
    dispose(){disposed=true;stop();context?.close().catch(()=>{});win?.removeEventListener('pointerdown',unlock);win?.removeEventListener('keydown',unlock);doc?.removeEventListener('visibilitychange',visibility);},
  };
}
