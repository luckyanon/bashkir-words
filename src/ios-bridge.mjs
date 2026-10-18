/** The native iPhone shell supplies this Promise-based WebKit message handler. */
export function installIOSBridge(win = window) {
  const handler = win.webkit?.messageHandlers?.bashkir;
  if (!handler) throw new Error('iPhone bridge is unavailable');
  const call = (operation, payload = {}) => handler.postMessage({ operation, ...payload });
  function listen(name, callback) {
    const wrapped = event => callback(event.detail);
    win.addEventListener(name, wrapped);
    const remove = () => win.removeEventListener(name, wrapped);
    return Promise.resolve({ remove });
  }
  const plugins = {
    Preferences: {
      get: async ({ key }) => ({ value: await call('get', { key }) }),
      set: ({ key, value }) => call('set', { key, value }),
      remove: ({ key }) => call('remove', { key }),
    },
    Haptics: { vibrate: () => call('haptic') },
    App: {
      addListener: (event, callback) => event === 'appStateChange'
        ? listen('bashkir-app-state', callback)
        : Promise.resolve({ remove() {} }),
    },
    BackgroundMotion: {
      addListener: (event, callback) => listen('bashkir-orientation', callback),
      start: () => call('motionStart'),
      stop: () => call('motionStop'),
    },
  };
  // Fetching custom URL schemes is inconsistent across WebKit releases. Read
  // bundled JSON/text through the bridge; scripts/images use the scheme handler.
  const originalFetch = win.fetch.bind(win);
  win.fetch = async (input, options) => {
    const url = new URL(typeof input === 'string' || input instanceof URL ? input : input.url, win.location.href);
    if (url.protocol !== 'bashkir:' || url.host !== 'app') return originalFetch(input, options);
    if ((options?.method || input?.method || 'GET').toUpperCase() !== 'GET') throw new TypeError('Local resources are read-only');
    try {
      const text = await call('readResource', { path: url.pathname });
      return new Response(text, { status: 200, headers: { 'Content-Type': url.pathname.endsWith('.json') ? 'application/json' : 'text/plain; charset=utf-8' } });
    } catch {
      return new Response('Not found', { status: 404 });
    }
  };
  win.BashkirNativePlugins = plugins;
  return plugins;
}
