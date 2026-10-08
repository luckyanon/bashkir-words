// Keep offline updates working even if a prior game version cannot load a save.
window.addEventListener('load', () => {
  if (!window.Capacitor?.isNativePlatform?.() && 'serviceWorker' in navigator && location.protocol !== 'file:') {
    navigator.serviceWorker.register('./sw.js').catch(() => {});
  }
});
