let engine;

export function hashSeed(value) {
  let state = 2166136261;
  for (const character of String(value)) {
    state = Math.imul(state ^ character.codePointAt(0), 16777619) >>> 0;
  }
  return state;
}

export async function installHardcoreLayoutWasm(bytes) {
  if (bytes === null) {
    engine = undefined;
    return false;
  }
  const { instance } = await WebAssembly.instantiate(bytes);
  const { memory, prepare_input, layout_words, output_ptr } = instance.exports;
  if (!(memory instanceof WebAssembly.Memory)
      || [prepare_input, layout_words, output_ptr].some(fn => typeof fn !== 'function')) {
    throw new TypeError('Unexpected Hardcore WebAssembly exports');
  }
  engine = instance.exports;
  return true;
}

export async function loadHardcoreLayoutWasm(fetcher = globalThis.fetch) {
  try {
    const response = await fetcher('./wasm/hardcore-layout.wasm');
    if (response.ok) return await installHardcoreLayoutWasm(await response.arrayBuffer());
  } catch {
    // The JavaScript layout also supports devices that cannot load WebAssembly.
  }
  return false;
}

export function createHardcoreLayoutWithWasm(source, seed) {
  if (!engine || !source.length) return undefined;

  let inputLength = 0;
  for (const { word } of source) inputLength += word.length + 1;

  try {
    // Each word starts with its length, followed by its Cyrillic UTF-16 code units.
    const inputPointer = engine.prepare_input(inputLength);
    if (!inputPointer) return undefined;
    const input = new Uint32Array(engine.memory.buffer, inputPointer, inputLength);
    let cursor = 0;
    for (const { word } of source) {
      input[cursor++] = word.length;
      for (let i = 0; i < word.length; i++) input[cursor++] = word.charCodeAt(i);
    }

    const count = engine.layout_words(inputLength, hashSeed(seed));
    if (count !== source.length) return undefined;
    // The Rust call can grow the memory, so create a fresh view for its output.
    const result = new Int32Array(engine.memory.buffer, engine.output_ptr(), 2 + count * 4);
    const [rows, cols] = result;
    const words = [];
    for (let offset = 2; offset < result.length; offset += 4) {
      const { tie, ...entry } = source[result[offset]];
      words.push({
        ...entry,
        row: result[offset + 1],
        col: result[offset + 2],
        direction: result[offset + 3] === 0 ? 'across' : 'down',
      });
    }
    return { rows, cols, words };
  } catch {
    engine = undefined;
    return undefined;
  }
}
