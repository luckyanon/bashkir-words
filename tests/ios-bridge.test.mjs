import test from 'node:test';
import assert from 'node:assert/strict';
import { installIOSBridge } from '../src/ios-bridge.mjs';
import { createCapacitorStorageAdapter } from '../src/platform.mjs';
import { createStorage } from '../src/storage.mjs';

function device(values = new Map()) {
  const events = new EventTarget();
  return {
    location: { href: 'bashkir://app/index.html' },
    addEventListener: events.addEventListener.bind(events),
    removeEventListener: events.removeEventListener.bind(events),
    dispatchEvent: events.dispatchEvent.bind(events),
    fetch: async () => { throw Error('Unexpected network request'); },
    webkit: { messageHandlers: { bashkir: { async postMessage({ operation, key, value, path }) {
      if (operation === 'get') return values.get(key) ?? null;
      if (operation === 'set') values.set(key, value);
      if (operation === 'remove') values.delete(key);
      if (operation === 'readResource') {
        if (path === '/data/example.json') return '{"word":"ӘСӘЙ"}';
        throw Error('Missing resource');
      }
      return null;
    } } } },
  };
}

test('iPhone progress survives a new webview and remains isolated from other devices', async () => {
  const disk = new Map();
  const storage = win => createStorage(createCapacitorStorageAdapter(installIOSBridge(win).Preferences));
  const save = { version: 2, currentLevel: 8, coins: 120, levels: {}, settings: { sound: false, animations: true } };
  await storage(device(disk)).save(save);
  assert.deepEqual(await storage(device(disk)).load(), save);
  assert.equal(await storage(device()).load(), null);
});

test('iPhone reads Unicode JSON from its bundle and reports missing files without the network', async () => {
  const win = device(); installIOSBridge(win);
  const response = await win.fetch('./data/example.json');
  assert.equal(response.ok, true);
  assert.deepEqual(await response.json(), { word: 'ӘСӘЙ' });
  assert.equal((await win.fetch('./data/missing.json')).status, 404);
  await assert.rejects(win.fetch('./data/example.json', { method: 'POST' }), /read-only/);
});

test('iPhone lifecycle listeners cancel when removed and motion delivers angles', async () => {
  const win = device(), plugins = installIOSBridge(win), seen = [];
  const listener = await plugins.App.addListener('appStateChange', value => seen.push(value));
  const event = new Event('bashkir-app-state'); event.detail = { isActive: false };
  win.dispatchEvent(event); listener.remove(); win.dispatchEvent(event);
  assert.deepEqual(seen, [{ isActive: false }]);
  const motion = await plugins.BackgroundMotion.addListener('orientation', value => seen.push(value));
  const tilt = new Event('bashkir-orientation'); tilt.detail = { beta: 12, gamma: -8 };
  win.dispatchEvent(tilt); motion.remove();
  assert.deepEqual(seen[1], tilt.detail);
});
