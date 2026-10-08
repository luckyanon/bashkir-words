import test from 'node:test';
import assert from 'node:assert/strict';
import { createStorage, createWebStorageAdapter, decodeSave, STORAGE_KEY, LEGACY_BACKUP_KEY, GENERATOR_V2_BACKUP_KEY } from '../src/storage.mjs';
import { createPlatform, createSound, createCapacitorStorageAdapter } from '../src/platform.mjs';

const save = coins => ({ version: 1, currentLevel: 1, coins, levels: { 1: { revealed: [], solved: [], bonus: [] } }, settings: { sound: false, animations: true } });
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));

test('storage round-trip, detection and clearing use a replaceable adapter', async () => {
  const storage = createStorage(createWebStorageAdapter(null));
  assert.equal(await storage.hasSave(), false);
  assert.equal(await storage.save(save(90)), true);
  assert.deepEqual(await storage.load(), save(90));
  assert.equal(await storage.hasSave(), true);
  assert.equal(storage.status.persistent, false);
  assert.equal(await storage.clear(), true);
  assert.equal(await storage.load(), null);
});

test('corrupt, huge and unsupported save payloads safely return null', async () => {
  for (const value of ['bad json', '{', '[]', 'null', JSON.stringify({ version: 7, levels: {} }), JSON.stringify({ version: 1, levels: [] }), ' '.repeat(2_000_001)]) {
    assert.equal(decodeSave(value), null);
    const storage = createStorage({ get: async () => value, set: async () => {}, remove: async () => {} });
    assert.equal(await storage.load(), null);
    assert(storage.status.error);
  }
});

test('asynchronous native saves are serialized so newer progress wins', async () => {
  let stored;
  const writes = [];
  const storage = createStorage({
    get: async () => stored ?? null,
    set: async (key, value) => { const data = JSON.parse(value); await delay(data.coins === 90 ? 15 : 1); writes.push(data.coins); stored = value; },
    remove: async () => { stored = null; },
  });
  assert.deepEqual(await Promise.all([storage.save(save(90)), storage.save(save(70)), storage.save(save(50))]), [true, true, true]);
  assert.deepEqual(writes, [90, 70, 50]);
  assert.equal((await storage.load()).coins, 50);
  await storage.clear();
  assert.equal(await storage.load(), null);
});

test('browser write denial reports failed persistence but keeps the in-memory session', async () => {
  const storage = createStorage(createWebStorageAdapter({
    getItem: () => null, setItem: () => { throw new Error('Quota exceeded'); }, removeItem: () => {},
  }));
  assert.equal(await storage.save(save(90)), false);
  assert.equal(storage.status.persistent, false);
  assert.equal(storage.status.error, 'Quota exceeded');
  assert.deepEqual(await storage.load(), save(90));
  assert.equal(await storage.save(save(70)), true);
  assert.deepEqual(await storage.load(), save(70));
});

test('Capacitor Preferences preserves the same storage contract', async () => {
  const memory = new Map();
  const adapter = createCapacitorStorageAdapter({
    get: async ({ key }) => ({ value: memory.get(key) ?? null }),
    set: async ({ key, value }) => memory.set(key, value),
    remove: async ({ key }) => memory.delete(key),
  });
  const storage = createStorage(adapter);
  await storage.save(save(90));
  assert.deepEqual(await storage.load(), save(90));
  assert.equal(storage.status.persistent, true);
});

test('legacy migration backs up the original bytes before saving v2 and never replaces that backup', async () => {
  const raw = JSON.stringify(save(217), null, 2);
  const memory = new Map([[STORAGE_KEY, raw]]);
  const writes = [];
  const storage = createStorage({ get: async key => memory.get(key) ?? null,
    set: async (key, value) => { writes.push(key); memory.set(key, value); },
    remove: async key => memory.delete(key),
  });
  assert.equal((await storage.load()).version, 1);
  const updated = { ...save(217), version: 2, maxUnlocked: 1, campaignSeed: 'backup-test', completedRanges: [] };
  assert.equal(await storage.save(updated), true);
  assert.equal(memory.get(LEGACY_BACKUP_KEY), raw);
  assert.deepEqual(writes, [LEGACY_BACKUP_KEY, STORAGE_KEY]);
  assert.equal((await storage.load()).version, 2);
  await storage.save({ ...updated, coins: 197 });
  assert.equal(memory.get(LEGACY_BACKUP_KEY), raw);
  await storage.clear();
  assert.equal(memory.get(LEGACY_BACKUP_KEY), raw);
});

test('a failed legacy backup leaves the original saved progress intact', async () => {
  const raw = JSON.stringify(save(217));
  const memory = new Map([[STORAGE_KEY, raw]]);
  const storage = createStorage({ get: async key => memory.get(key) ?? null,
    set: async (key, value) => { if (key === LEGACY_BACKUP_KEY) throw new Error('Backup quota exceeded'); memory.set(key, value); },
    remove: async key => memory.delete(key),
  });
  assert.equal(await storage.save({ ...save(217), version: 2 }), false);
  assert.equal(memory.get(STORAGE_KEY), raw);
  assert.equal(storage.status.error, 'Backup quota exceeded');
});

test('generator migration preserves exact v2 bytes and keeps the backup across later saves and reset', async () => {
  const previous = { ...save(217), version: 2, campaignSeed: 'retained-campaign', generatorVersion: 2, maxUnlocked: 8 };
  const raw = JSON.stringify(previous, null, 2);
  const memory = new Map([[STORAGE_KEY, raw]]);
  const writes = [];
  const storage = createStorage({ get: async key => memory.get(key) ?? null,
    set: async (key, value) => { writes.push(key); memory.set(key, value); },
    remove: async key => memory.delete(key),
  });
  const current = { ...previous, generatorVersion: 3, legacyThroughLevel: 8 };
  assert.equal(await storage.save(current), true);
  assert.deepEqual(writes, [GENERATOR_V2_BACKUP_KEY, STORAGE_KEY]);
  assert.equal(memory.get(GENERATOR_V2_BACKUP_KEY), raw);
  assert.deepEqual(await storage.load(), current);
  await storage.save({ ...current, coins: 197 });
  await storage.clear();
  await storage.save({ ...current, legacyThroughLevel: 0 });
  assert.equal(memory.get(GENERATOR_V2_BACKUP_KEY), raw);
});

test('a failed generator backup cannot replace the original campaign', async () => {
  const previous = { ...save(217), version: 2, generatorVersion: 2 };
  const raw = JSON.stringify(previous);
  const memory = new Map([[STORAGE_KEY, raw]]);
  const storage = createStorage({ get: async key => memory.get(key) ?? null,
    set: async (key, value) => { if (key === GENERATOR_V2_BACKUP_KEY) throw new Error('Generator backup denied'); memory.set(key, value); },
    remove: async key => memory.delete(key),
  });
  assert.equal(await storage.save({ ...previous, generatorVersion: 3 }), false);
  assert.equal(memory.get(STORAGE_KEY), raw);
  assert.equal(storage.status.error, 'Generator backup denied');
});

test('platform has no browser requirement and native background/back hooks cancel input', async () => {
  const handlers = {};
  let cancelled = 0;
  let back = 0;
  let removed = 0;
  const platform = createPlatform({
    onCancel: () => cancelled++, onBack: () => back++, settings: () => ({ sound: false }),
    plugins: { App: { addListener: async (name, handler) => { handlers[name] = handler; return { remove: () => removed++ }; } } },
  });
  await Promise.resolve();
  handlers.appStateChange({ isActive: false });
  handlers.backButton();
  assert.equal(cancelled, 2);
  assert.equal(back, 1);
  platform.sound('victory');
  platform.dispose();
  assert.equal(removed, 2);
  const sound = createSound({ AudioContext: undefined });
  assert.doesNotThrow(() => { sound.play(); sound.setEnabled(false); sound.dispose(); });
});
