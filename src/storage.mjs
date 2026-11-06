export const STORAGE_KEY = 'bashkir-words:save';
export const STORAGE_VERSION = 2;
export const LEGACY_BACKUP_KEY = `${STORAGE_KEY}:legacy-v1`;
export const GENERATOR_V2_BACKUP_KEY = `${STORAGE_KEY}:generator-v2`;

function browserStorage() {
  try { return globalThis.localStorage; } catch { return null; }
}

/** The same asynchronous contract also works with Capacitor Preferences. */
export function createWebStorageAdapter(storage = browserStorage()) {
  const memory = new Map();
  let persistent = !!storage;
  return {
    get persistent() { return persistent; },
    async get(key) {
      if (persistent) try { return storage.getItem(key); } catch { persistent = false; }
      return memory.get(key) ?? null;
    },
    async set(key, value) {
      memory.set(key, value);
      if (persistent) try { storage.setItem(key, value); } catch (error) { persistent = false; throw error; }
    },
    async remove(key) {
      memory.delete(key);
      if (persistent) try { storage.removeItem(key); } catch (error) { persistent = false; throw error; }
    },
  };
}

export function decodeSave(value) {
  try {
    if (typeof value !== 'string' || value.length > 2_000_000) return null;
    const state = JSON.parse(value);
    if (!state || typeof state !== 'object' || Array.isArray(state) || ![1, STORAGE_VERSION].includes(state.version) ||
      !state.levels || typeof state.levels !== 'object' || Array.isArray(state.levels)) return null;
    return state;
  } catch { return null; }
}

export function createStorage(adapter = createWebStorageAdapter(), { key = STORAGE_KEY, backupKey = `${key}:legacy-v1`, generatorBackupKey = `${key}:generator-v2` } = {}) {
  const status = { persistent: adapter.persistent !== false, error: null };
  let queue = Promise.resolve();
  async function load() {
    await queue;
    try {
      const raw = await adapter.get(key);
      status.persistent = adapter.persistent !== false;
      const state = decodeSave(raw);
      status.error = raw != null && !state ? 'Повреждённое или неподдерживаемое сохранение' : null;
      return state;
    } catch (error) { status.error = error?.message || 'Не удалось прочитать сохранение'; return null; }
  }
  function save(state) {
    let value;
    try {
      value = JSON.stringify(state);
      if (!decodeSave(value)) throw new TypeError('Unsupported save format');
    } catch (error) { status.error = error.message; return Promise.resolve(false); }
    // Serialize writes, including native asynchronous saves, to prevent old progress winning a race.
    const snapshot = JSON.parse(value);
    const operation = queue.then(async () => {
      const state = snapshot;
      try {
        const previous = await adapter.get(key);
        const priorState = decodeSave(previous);
        if(state.datasetRevision){const prior=previous,old=priorState;if(old&&old.datasetRevision!==state.datasetRevision){const revisionBackup=`${key}:before-dataset-${state.datasetRevision}`;if(!(await adapter.get(revisionBackup)))await adapter.set(revisionBackup,prior);}}
        // Back up the exact old save before the first migration write. A failed
        // backup leaves the original key intact and reports a save failure.
        if (state.version === STORAGE_VERSION) {
          if (priorState?.version === 1 && await adapter.get(backupKey) == null) await adapter.set(backupKey, previous);
          if (priorState?.version === STORAGE_VERSION && priorState.generatorVersion === 2 && state.generatorVersion >= 3
            && await adapter.get(generatorBackupKey) == null) await adapter.set(generatorBackupKey, previous);
          if (priorState?.version === STORAGE_VERSION && priorState.generatorVersion === 3 && state.generatorVersion >= 4
            && await adapter.get(`${key}:generator-v3`) == null) await adapter.set(`${key}:generator-v3`, previous);
        }
        if(state.rackRepairVersion===1){const prior=priorState;if(prior && prior.rackRepairVersion!==1 && await adapter.get(`${key}:before-rack-repair`)==null)await adapter.set(`${key}:before-rack-repair`,previous);}
        if(state.generatorVersion>=5){const prior=priorState;if(prior && prior.generatorVersion!==state.generatorVersion && await adapter.get(`${key}:before-nouns-v${state.generatorVersion}`)==null)await adapter.set(`${key}:before-nouns-v${state.generatorVersion}`,previous);}
        if(state.layoutVersion===2){const prior=priorState;if(prior && (prior.layoutVersion??1)<2 && await adapter.get(`${key}:before-layout-v2`)==null)await adapter.set(`${key}:before-layout-v2`,previous);}
        if(state.wordRevision===1){const prior=priorState;if(prior && prior.wordRevision!==1 && await adapter.get(`${key}:before-local-words-v1`)==null)await adapter.set(`${key}:before-local-words-v1`,previous);}
        await adapter.set(key, value); status.persistent = adapter.persistent !== false; status.error = null; return true;
      }
      catch (error) { status.persistent = adapter.persistent !== false; status.error = error?.message || 'Не удалось сохранить прогресс'; return false; }
    });
    queue = operation.then(() => undefined);
    return operation;
  }
  function clear() {
    const operation = queue.then(async () => {
      try { await adapter.remove(key); status.error = null; return true; }
      catch (error) { status.error = error?.message || 'Не удалось очистить сохранение'; return false; }
    });
    queue = operation.then(() => undefined);
    return operation;
  }
  return { load, save, clear, status, hasSave: async () => !!(await load()) };
}

