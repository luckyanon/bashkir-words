/** Tile IDs identify physical buttons, including separate copies of one letter. */
export class LetterPath {
  constructor(tiles) { this.tiles = tiles; this.ids = []; }
  visit(id) {
    if (!this.tiles.some(tile => tile.id === id)) return false;
    if (this.ids.at(-1) === id) return false;
    if (this.ids.length > 1 && this.ids.at(-2) === id) { this.ids.pop(); return true; }
    if (this.ids.includes(id)) return false;
    this.ids.push(id); return true;
  }
  clear() { this.ids = []; }
  get word() { return this.ids.map(id => this.tiles.find(tile => tile.id === id).letter).join(''); }
}

/** Drag submits on release. Native Space clicks select; Enter submits; Escape cancels. */
export function bindWheel(wheel, { letters, tiles = letters?.map((letter, id) => ({ letter, id })), onSubmit, onSelect = () => {}, onPreview }) {
  const path = new LetterPath(tiles);
  const buttons = [...wheel.querySelectorAll('[data-tile]')];
  const tileIds = new Map(tiles.map(tile => [String(tile.id), tile.id]));
  const byId = new Map(buttons.map(button => [tileIds.get(button.dataset.tile), button]));
  const svgLine = wheel.querySelector('polyline');
  const preview = document.querySelector('#word-preview');
  const cleanups = [];
  let pointer = null, last = null;
  function listen(element, type, handler, options) {
    element.addEventListener(type, handler, options);
    cleanups.push(() => element.removeEventListener(type, handler, options));
  }
  function paint() {
    buttons.forEach(button => {
      const selected = path.ids.includes(tileIds.get(button.dataset.tile));
      button.classList.toggle('selected', selected);
      button.setAttribute('aria-pressed', String(selected));
    });
    if (onPreview) onPreview(path.word);
    else if (preview) {
      preview.textContent = path.word || '· · ·';
      preview.classList.toggle('has-word', !!path.word);
    }
    const rect = wheel.getBoundingClientRect();
    svgLine?.setAttribute('points', path.ids.map(id => {
      const r = byId.get(id).getBoundingClientRect();
      return `${(r.x + r.width / 2 - rect.x) / rect.width * 100},${(r.y + r.height / 2 - rect.y) / rect.height * 100}`;
    }).join(' '));
  }
  function visit(id) { if (path.visit(id)) { paint(); onSelect(); } }
  function hit(x, y) {
    let closest, distance = Infinity;
    for (const button of buttons) {
      const r = button.getBoundingClientRect();
      const d = Math.hypot(x - (r.x + r.width / 2), y - (r.y + r.height / 2));
      if (d < Math.max(r.width, r.height) * .575 && d < distance) {
        closest = tileIds.get(button.dataset.tile); distance = d;
      }
    }
    if (closest !== undefined) visit(closest);
  }
  function traverse(x, y) {
    if (!last) return;
    const steps = Math.max(1, Math.ceil(Math.hypot(x - last.x, y - last.y) / 8));
    for (let i = 1; i <= steps; i++) hit(last.x + (x - last.x) * i / steps, last.y + (y - last.y) * i / steps);
    last = { x, y };
  }
  function release() {
    const id = pointer;
    pointer = null; last = null;
    if (id !== null && wheel.hasPointerCapture?.(id)) wheel.releasePointerCapture(id);
  }
  function cancel() { release(); path.clear(); paint(); }
  function submit() {
    const word = path.ids.length >= 2 ? path.word : '';
    cancel();
    if (word) onSubmit(word);
  }
  listen(wheel, 'pointerdown', event => {
    if (event.button !== 0 || event.isPrimary === false) return;
    cancel();
    const button = event.target.closest?.('[data-tile]');
    const id = button && tileIds.get(button.dataset.tile);
    if (id === undefined || !byId.has(id)) return;
    event.preventDefault();
    pointer = event.pointerId; last = { x: event.clientX, y: event.clientY };
    // A pointer can be cancelled between dispatch and capture, especially on touch.
    try { wheel.setPointerCapture(pointer); } catch { cancel(); return; }
    visit(id);
  });
  listen(wheel, 'pointermove', event => {
    if (event.pointerId !== pointer) return;
    event.preventDefault(); traverse(event.clientX, event.clientY);
  });
  listen(wheel, 'pointerup', event => {
    if (event.pointerId !== pointer) return;
    event.preventDefault(); traverse(event.clientX, event.clientY); submit();
  });
  listen(wheel, 'pointercancel', event => { if (event.pointerId === pointer) cancel(); });
  listen(wheel, 'lostpointercapture', event => { if (event.pointerId === pointer) cancel(); });
  listen(wheel, 'contextmenu', event => event.preventDefault());
  buttons.forEach(button => listen(button, 'click', event => {
    if (event.detail === 0) visit(tileIds.get(button.dataset.tile));
    else event.preventDefault();
  }));
  listen(wheel, 'keydown', event => {
    if (event.key === 'Enter') { event.preventDefault(); if (!event.repeat) submit(); }
    else if (event.key === 'Escape') { event.preventDefault(); cancel(); }
  });
  listen(window, 'resize', paint);
  listen(window, 'blur', cancel);
  listen(document, 'visibilitychange', () => { if (document.hidden) cancel(); });
  paint();
  return { cancel, clear: cancel, submit, path, refresh: paint, dispose: () => { cancel(); cleanups.forEach(fn => fn()); } };
}

