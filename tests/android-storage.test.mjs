import test from 'node:test';
import assert from 'node:assert/strict';
import { createCapacitorStorageAdapter } from '../src/platform.mjs';
import { createStorage } from '../src/storage.mjs';

test('Android devices have independent empty saves and retain their own progress after restart', async () => {
  const device = () => {
    const values = new Map();
    return {
      get: async ({ key }) => ({ value: values.get(key) ?? null }),
      set: async ({ key, value }) => { values.set(key, value); },
      remove: async ({ key }) => { values.delete(key); },
    };
  };
  const first = device(), second = device();
  const storage = preferences => createStorage(createCapacitorStorageAdapter(preferences));
  assert.equal(await storage(first).load(), null);
  const progress = { version: 2, currentLevel: 8, coins: 120, levels: {}, settings: { sound: true, animations: true } };
  await storage(first).save(progress);
  assert.equal(await storage(second).load(), null);
  assert.deepEqual(await storage(first).load(), progress);
});
