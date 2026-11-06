import { createWebStorageAdapter } from './storage.mjs';

/** Short original tones generated on-device; no audio download or player dependency. */
export function createSound({ enabled = true, AudioContext = globalThis.AudioContext || globalThis.webkitAudioContext } = {}) {
  let context;
  let disposed = false;
  const active = new Set();
  function play(type = 'select') {
    if (!enabled || disposed || !AudioContext) return;
    try {
      context ||= new AudioContext();
      const tones = { select: [392], target: [523, 659], bonus: [659, 784], repeat: [784, 659], invalid: [196], hint: [440, 523], victory: [523, 659, 784, 1047] };
      const frequencies = tones[type] || tones.select;
      const begin = () => frequencies.forEach((frequency, index) => {
        if (disposed || !enabled) return;
        const oscillator = context.createOscillator();
        const gain = context.createGain();
        const time = context.currentTime + index * (type==='repeat'?.14:.08);
        oscillator.type = 'sine';
        oscillator.frequency.value = frequency;
        if(type==='invalid')oscillator.frequency.exponentialRampToValueAtTime(155,time+.1);
        gain.gain.setValueAtTime(.0001, time);
        gain.gain.exponentialRampToValueAtTime(type==='invalid'?.025:type==='repeat'?.075:.055, time + .01);
        gain.gain.exponentialRampToValueAtTime(.0001, time + .11);
        oscillator.connect(gain); gain.connect(context.destination);
        active.add(oscillator);
        oscillator.start(time); oscillator.stop(time + .12);
        oscillator.onended = () => { active.delete(oscillator); oscillator.disconnect(); gain.disconnect(); };
      });
      if (context.state === 'suspended') context.resume().then(begin).catch(() => {}); else begin();
    } catch { /* Missing or blocked audio never changes the game. */ }
  }
  function stop() {
    for (const oscillator of active) try { oscillator.stop(); } catch {}
    active.clear();
    try { if (context?.state === 'running') context.suspend().catch(() => {}); } catch {}
  }
  return {
    play, stop,
    setEnabled(value) { enabled = !!value; if (!enabled) stop(); },
    dispose() { disposed = true; context?.close?.().catch?.(() => {}); },
  };
}

export function createCapacitorStorageAdapter(preferences) {
  if (!preferences?.get || !preferences?.set || !preferences?.remove) throw new TypeError('Capacitor Preferences adapter is missing');
  return {
    persistent: true,
    get: async key => (await preferences.get({ key })).value ?? null,
    set: (key, value) => preferences.set({ key, value }),
    remove: key => preferences.remove({ key }),
  };
}

/** Native plugins can be injected after the Android wrapper is added. */
export function createPlatform({ onCancel = () => {}, onBack = () => {}, settings = () => ({ sound: true }), plugins = {} } = {}) {
  const sound = createSound({ enabled: typeof settings === 'function' ? settings().sound : settings.sound });
  const removers = [];
  let disposed = false;
  function currentSettings() { return typeof settings === 'function' ? settings() : settings; }
  function cancel() { onCancel(); sound.stop(); }
  if (typeof document !== 'undefined') {
    const visibility = () => { if (document.hidden) cancel(); };
    document.addEventListener('visibilitychange', visibility);
    removers.push(() => document.removeEventListener('visibilitychange', visibility));
  }
  if (typeof window !== 'undefined') {
    window.addEventListener('blur', cancel);
    window.addEventListener('pagehide', cancel);
    removers.push(() => { window.removeEventListener('blur', cancel); window.removeEventListener('pagehide', cancel); });
  }
  if (plugins.App?.addListener) {
    for (const [event, handler] of [
      ['appStateChange', value => { if (!value.isActive) cancel(); }],
      ['backButton', () => { cancel(); onBack(); }],
    ]) {
      Promise.resolve(plugins.App.addListener(event, handler)).then(listener => {
        if (disposed) listener.remove(); else removers.push(() => listener.remove());
      }).catch(() => {});
    }
  }
  return {
    storageAdapter: plugins.Preferences ? createCapacitorStorageAdapter(plugins.Preferences) : createWebStorageAdapter(),
    sound(type) { sound.setEnabled(currentSettings().sound !== false); sound.play(type); },
    vibrate(duration = 15) {
      try {
        if (plugins.Haptics?.vibrate) Promise.resolve(plugins.Haptics.vibrate({ duration })).catch(() => {});
        else globalThis.navigator?.vibrate?.(duration);
      } catch { /* Haptics are optional, including on devices that deny access. */ }
    },
    cancel,
    dispose() { disposed = true; cancel(); removers.forEach(remove => remove()); sound.dispose(); },
  };
}


