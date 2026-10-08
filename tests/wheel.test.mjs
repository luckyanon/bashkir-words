import test from 'node:test';
import assert from 'node:assert/strict';
import { bindWheel } from '../src/gestures.mjs';

class Element extends EventTarget {
  constructor({ id, x = 0, y = 0, width = 100, height = 100 } = {}) {
    super();
    this.dataset = id === undefined ? {} : { tile: String(id) };
    this.rect = { x, y, width, height };
    this.attributes = new Map();
    this.classes = new Set();
    this.classList = {
      toggle: (name, force) => {
        const add = force === undefined ? !this.classes.has(name) : force;
        if (add) this.classes.add(name); else this.classes.delete(name);
        return add;
      },
      contains: name => this.classes.has(name),
    };
    this.textContent = '';
  }
  closest(selector) { return selector === '[data-tile]' && this.dataset.tile !== undefined ? this : null; }
  setAttribute(name, value) { this.attributes.set(name, String(value)); }
  getAttribute(name) { return this.attributes.get(name) ?? null; }
  getBoundingClientRect() { return { ...this.rect }; }
}

function event(target, type, values = {}, originalTarget = target) {
  const value = new Event(type, { cancelable: true });
  Object.assign(value, { pointerId: 1, pointerType: 'mouse', button: 0, isPrimary: true, clientX: 0, clientY: 0, detail: 1 }, values);
  Object.defineProperty(value, 'target', { value: originalTarget });
  target.dispatchEvent(value);
  return value;
}

function fixture(t, { tiles, buttonIds } = {}) {
  const oldWindow = globalThis.window;
  const oldDocument = globalThis.document;
  const window = new EventTarget();
  const document = new EventTarget();
  document.hidden = false;
  const wheel = new Element();
  const positions = [[50, 10], [90, 50], [50, 90], [10, 50]];
  const ids = buttonIds || tiles?.map(tile => tile.id) || [0, 1, 2, 3];
  const buttons = positions.map(([x, y], slot) => new Element({ id: ids[slot], x: x - 10, y: y - 10, width: 20, height: 20 }));
  const line = new Element();
  const preview = new Element();
  const captured = new Set();
  wheel.querySelectorAll = selector => selector === '[data-tile]' ? buttons : [];
  wheel.querySelector = selector => selector === 'polyline' ? line : null;
  wheel.setPointerCapture = id => captured.add(id);
  wheel.hasPointerCapture = id => captured.has(id);
  wheel.releasePointerCapture = id => {
    captured.delete(id);
    // Browsers emit this on release; submission must survive it.
    event(wheel, 'lostpointercapture', { pointerId: id });
  };
  document.querySelector = selector => selector === '#word-preview' ? preview : null;
  globalThis.window = window;
  globalThis.document = document;
  const submitted = [];
  let selections = 0;
  const binding = bindWheel(wheel, {
    ...(tiles ? { tiles } : { letters: ['А', 'Л', 'М', 'А'] }),
    onSubmit: word => submitted.push(word), onSelect: () => selections++,
  });
  t.after(() => {
    binding.dispose();
    if (oldWindow === undefined) delete globalThis.window; else globalThis.window = oldWindow;
    if (oldDocument === undefined) delete globalThis.document; else globalThis.document = oldDocument;
  });
  const pointer = (type, slot, values = {}) => {
    const rect = buttons[slot].rect;
    return event(wheel, type, { clientX: rect.x + rect.width / 2, clientY: rect.y + rect.height / 2, ...values }, type === 'pointerdown' ? buttons[slot] : wheel);
  };
  return { window, document, wheel, buttons, preview, line, captured, submitted, binding, pointer, get selections() { return selections; } };
}

test('mouse drag submits once on release, paints tiles and line, then clears', t => {
  const f = fixture(t);
  assert.equal(f.pointer('pointerdown', 0).defaultPrevented, true);
  assert.equal(f.preview.textContent, 'А', 'the touched tile is selected immediately');
  assert.ok(f.captured.has(1));
  f.pointer('pointermove', 1);
  assert.equal(f.preview.textContent, 'АЛ');
  assert.equal(f.buttons[0].getAttribute('aria-pressed'), 'true');
  assert.equal(f.buttons[1].classList.contains('selected'), true);
  assert.equal(f.line.getAttribute('points'), '50,10 90,50');
  f.pointer('pointermove', 2);
  f.pointer('pointermove', 3);
  assert.equal(f.preview.textContent, 'АЛМА');
  f.pointer('pointerup', 3);
  assert.deepEqual(f.submitted, ['АЛМА']);
  assert.equal(f.captured.has(1), false);
  assert.equal(f.preview.textContent, '· · ·');
  assert.equal(f.line.getAttribute('points'), '');
  assert.ok(f.buttons.every(button => button.getAttribute('aria-pressed') === 'false'));
  f.pointer('pointerup', 3);
  assert.deepEqual(f.submitted, ['АЛМА']);
});

test('touch drag supports separate repeated letters, reverse backtracking and no tile reuse', t => {
  const f = fixture(t);
  const touch = { pointerId: 7, pointerType: 'touch' };
  f.pointer('pointerdown', 0, touch);
  for (const id of [1, 2, 3]) f.pointer('pointermove', id, touch);
  assert.equal(f.preview.textContent, 'АЛМА');
  f.pointer('pointermove', 2, touch);
  assert.equal(f.preview.textContent, 'АЛМ');
  f.pointer('pointermove', 1, touch);
  assert.equal(f.preview.textContent, 'АЛ');
  for (const id of [2, 3]) f.pointer('pointermove', id, touch);
  f.pointer('pointermove', 0, touch);
  assert.equal(f.preview.textContent, 'АЛМА');
  assert.deepEqual(f.binding.path.ids, [0, 1, 2, 3]);
  f.pointer('pointerup', 0, touch);
  assert.deepEqual(f.submitted, ['АЛМА']);
});

test('a single tile release clears quietly and pointer taps never accumulate', t => {
  const f = fixture(t);
  for (const slot of [0, 1, 2, 3]) {
    f.pointer('pointerdown', slot, { pointerType: 'touch' });
    f.pointer('pointerup', slot, { pointerType: 'touch' });
    assert.equal(event(f.buttons[slot], 'click', { detail: 1 }).defaultPrevented, true);
    assert.equal(f.preview.textContent, '· · ·');
    assert.deepEqual(f.binding.path.ids, []);
  }
  f.binding.submit();
  assert.deepEqual(f.submitted, []);
});

test('the final release position completes a word even when its last move was skipped', t => {
  const f = fixture(t);
  f.pointer('pointerdown', 0);
  f.pointer('pointerup', 1);
  assert.deepEqual(f.submitted, ['АЛ']);
});

test('a new press cancels the previous gesture and begins a fresh physical path', t => {
  const f = fixture(t);
  f.pointer('pointerdown', 0);
  f.pointer('pointermove', 1);
  f.pointer('pointerdown', 2, { pointerId: 5 });
  assert.deepEqual(f.binding.path.ids, [2]);
  assert.equal(f.captured.has(1), false);
  f.pointer('pointerup', 1);
  assert.deepEqual(f.binding.path.ids, [2]);
  f.pointer('pointermove', 3, { pointerId: 5 });
  f.pointer('pointerup', 3, { pointerId: 5 });
  assert.deepEqual(f.submitted, ['МА']);
});

test('capture loss, cancellation, background suspension and explicit clearing never submit', t => {
  const f = fixture(t);
  for (const type of ['pointercancel', 'lostpointercapture', 'blur', 'hidden', 'explicit', 'background']) {
    f.pointer('pointerdown', 0);
    f.pointer('pointermove', 1);
    assert.equal(f.preview.textContent, 'АЛ');
    if (type === 'explicit') f.binding.cancel();
    else if (type === 'blur') event(f.window, 'blur');
    else if (type === 'hidden') {
      f.document.hidden = true;
      event(f.document, 'visibilitychange');
      f.document.hidden = false;
    } else if (type === 'background') event(f.wheel, 'pointerdown');
    else event(f.wheel, type);
    assert.equal(f.preview.textContent, '· · ·', type);
    assert.deepEqual(f.binding.path.ids, [], type);
    f.pointer('pointermove', 2);
    f.pointer('pointerup', 2);
    assert.deepEqual(f.submitted, [], type);
  }
});

test('native Space clicks select, Enter submits without activating a letter, Escape clears', t => {
  const f = fixture(t);
  for (const slot of [0, 1, 2, 3]) event(f.buttons[slot], 'click', { detail: 0 });
  assert.equal(f.preview.textContent, 'АЛМА');
  assert.equal(f.selections, 4);
  const enter = event(f.wheel, 'keydown', { key: 'Enter' }, f.buttons[3]);
  assert.equal(enter.defaultPrevented, true, 'prevent the focused button native Enter click');
  assert.deepEqual(f.submitted, ['АЛМА']);
  assert.deepEqual(f.binding.path.ids, []);
  assert.equal(f.selections, 4);
  event(f.buttons[0], 'click', { detail: 0 });
  event(f.wheel, 'keydown', { key: 'Enter', repeat: true }, f.buttons[0]);
  assert.deepEqual(f.binding.path.ids, [0], 'held Enter does not resubmit or clear a new path');
  assert.equal(event(f.wheel, 'keydown', { key: 'Escape' }).defaultPrevented, true);
  assert.deepEqual(f.binding.path.ids, []);
  event(f.wheel, 'keydown', { key: 'Enter' });
  assert.deepEqual(f.submitted, ['АЛМА']);
});

test('shuffle retains nonsequential IDs and uses moved button bounds for hits and SVG', t => {
  const tiles = [{ id: 42, letter: 'М' }, { id: 9, letter: 'А' }, { id: 70, letter: 'А' }, { id: 3, letter: 'Л' }];
  const f = fixture(t, { tiles, buttonIds: [70, 3, 42, 9] });
  event(f.buttons[0], 'click', { detail: 0 });
  event(f.buttons[1], 'click', { detail: 0 });
  assert.equal(f.line.getAttribute('points'), '50,10 90,50');
  const before = f.buttons.map(button => ({ ...button.rect }));
  f.buttons.forEach((button, slot) => { button.rect = before[(slot + 2) % 4]; });
  f.binding.refresh();
  assert.equal(f.line.getAttribute('points'), '50,90 10,50');
  f.binding.clear();
  f.pointer('pointerdown', 0);
  for (const slot of [1, 2, 3]) f.pointer('pointermove', slot);
  assert.deepEqual(f.binding.path.ids, [70, 3, 42, 9]);
  assert.equal(f.preview.textContent, 'АЛМА');
  assert.equal(f.line.getAttribute('points'), '50,90 10,50 50,10 90,50');
  f.pointer('pointerup', 3);
  assert.deepEqual(f.submitted, ['АЛМА']);
});

test('secondary input is ignored and disposal removes wheel and background handlers', t => {
  const f = fixture(t);
  assert.equal(f.pointer('pointerdown', 0, { button: 2 }).defaultPrevented, false);
  assert.equal(f.pointer('pointerdown', 0, { isPrimary: false }).defaultPrevented, false);
  assert.deepEqual(f.binding.path.ids, []);
  f.pointer('pointerdown', 0);
  f.pointer('pointermove', 1, { pointerId: 2 });
  f.pointer('pointerup', 1, { pointerId: 2 });
  event(f.wheel, 'pointercancel', { pointerId: 2 });
  event(f.wheel, 'lostpointercapture', { pointerId: 2 });
  assert.deepEqual(f.binding.path.ids, [0]);
  assert.equal(event(f.wheel, 'contextmenu').defaultPrevented, true);
  f.binding.dispose();
  assert.deepEqual(f.binding.path.ids, []);
  for (const slot of [0, 1]) event(f.buttons[slot], 'click', { detail: 0 });
  f.pointer('pointerdown', 0);
  f.pointer('pointermove', 1);
  f.pointer('pointerup', 1);
  assert.deepEqual(f.submitted, []);
  assert.deepEqual(f.binding.path.ids, []);
  assert.equal(event(f.wheel, 'contextmenu').defaultPrevented, false);
  assert.equal(event(f.wheel, 'keydown', { key: 'Enter' }).defaultPrevented, false);
  assert.equal(f.captured.size, 0);
});
