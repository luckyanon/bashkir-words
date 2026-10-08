import test from 'node:test';
import assert from 'node:assert/strict';
import { LetterPath } from '../src/gestures.mjs';

const tiles = [
  { id: 0, letter: 'А' }, { id: 1, letter: 'Л' }, { id: 2, letter: 'М' }, { id: 3, letter: 'А' },
];

test('separate identical letter tiles form АЛМА and a physical tile cannot be reused', () => {
  const path = new LetterPath(tiles);
  for (const id of [0, 1, 2, 3]) assert.equal(path.visit(id), true);
  assert.equal(path.word, 'АЛМА');
  assert.equal(path.visit(0), false);
  assert.deepEqual(path.ids, [0, 1, 2, 3]);
  assert.equal(path.word, 'АЛМА');
});

test('backtracking removes exactly the last tile and supports repeated reverse movement', () => {
  const path = new LetterPath(tiles);
  for (const id of [0, 1, 2, 3]) path.visit(id);
  assert.equal(path.visit(2), true);
  assert.equal(path.word, 'АЛМ');
  assert.equal(path.visit(1), true);
  assert.equal(path.word, 'АЛ');
  assert.equal(path.visit(0), true);
  assert.equal(path.word, 'А');
  assert.deepEqual(path.ids, [0]);
});

test('revisiting the current tile or an unknown tile leaves the path unchanged', () => {
  const path = new LetterPath(tiles);
  assert.equal(path.visit(0), true);
  assert.equal(path.visit(0), false);
  assert.equal(path.visit(99), false);
  assert.deepEqual(path.ids, [0]);
  path.clear();
  assert.equal(path.word, '');
  assert.deepEqual(path.ids, []);
  assert.equal(path.visit(0), true);
});

test('Bashkir letters remain distinct during letter selection', () => {
  const path = new LetterPath([... 'ӨОҠКӘАҘЗ'].map((letter, id) => ({ id, letter })));
  for (let id = 0; id < 8; id++) path.visit(id);
  assert.equal(path.word, 'ӨОҠКӘАҘЗ');
});

test('stable nonsequential physical IDs preserve letters after tile array order changes', () => {
  const shuffled = [{ id: 'third', letter: 'А' }, { id: 'first', letter: 'А' }, { id: 'second', letter: 'Л' }];
  const path = new LetterPath(shuffled);
  for (const id of ['first', 'second', 'third']) path.visit(id);
  assert.equal(path.word, 'АЛА');
  assert.deepEqual(path.ids, ['first', 'second', 'third']);
});
