use std::cell::RefCell;

thread_local! {
    static INPUT: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    static OUTPUT: RefCell<Vec<i32>> = const { RefCell::new(Vec::new()) };
}

// The host writes the input into this buffer before calling layout_words.
#[unsafe(no_mangle)]
pub extern "C" fn prepare_input(length: u32) -> *mut u32 {
    INPUT.with_borrow_mut(|input| {
        input.resize(length as usize, 0);
        input.as_mut_ptr()
    })
}

// Input consists of repeated [length, ...Cyrillic UTF-16 units] records.
// The output is [rows, cols, ...[source index, row, col, direction] records].
#[unsafe(no_mangle)]
pub extern "C" fn layout_words(input_length: u32, seed: u32) -> u32 {
    INPUT.with_borrow(|input| {
        OUTPUT.with_borrow_mut(|output| {
            output.clear();
            if input.len() == input_length as usize {
                create_layout(input, seed, output)
            } else {
                0
            }
        })
    })
}

// This pointer remains valid until the next layout_words call.
#[unsafe(no_mangle)]
pub extern "C" fn output_ptr() -> *const i32 {
    OUTPUT.with_borrow(|output| output.as_ptr())
}

struct Random(u32);

impl Random {
    // Mulberry32, seeded by the campaign's existing JavaScript string hash.
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(0x6D2B79F5);
        let mut next = self.0;
        next = (next ^ (next >> 15)).wrapping_mul(next | 1);
        next ^= next.wrapping_add((next ^ (next >> 7)).wrapping_mul(next | 61));
        next ^ (next >> 14)
    }

    fn fraction(&mut self) -> f64 {
        f64::from(self.next()) / 4294967296.0
    }
}

#[derive(Clone, Copy)]
struct Position {
    row: i32,
    col: i32,
}

#[derive(Clone, Copy)]
enum Direction {
    Across = 0,
    Down = 1,
}

impl Direction {
    fn bit(self) -> u16 {
        0x100 << self as usize
    }

    fn perpendicular(self) -> Self {
        match self {
            Self::Across => Self::Down,
            Self::Down => Self::Across,
        }
    }

    fn offset(self, position: Position, cells: i32) -> Position {
        let down = self as i32;
        Position {
            row: position.row + cells * down,
            col: position.col + cells * (1 - down),
        }
    }
}

#[derive(Clone, Copy)]
struct Placement {
    start: Position,
    direction: Direction,
}

impl Placement {
    fn position(self, offset: i32) -> Position {
        self.direction.offset(self.start, offset)
    }
}

struct Word {
    index: i32,
    letters: Vec<u8>,
    tie: u32,
}

struct Board {
    radius: i32,
    cells: Vec<u16>,
    seen: Vec<[u32; 2]>,
    round: u32,
}

impl Board {
    fn new() -> Self {
        let radius: i32 = 16;
        let area = (radius * 2).pow(2) as usize;
        Self {
            radius,
            cells: vec![0; area],
            seen: vec![[0; 2]; area],
            round: 0,
        }
    }

    // Pad the low edge so even candidate starts offset from an occupied cell fit.
    fn reserve(&mut self, start: Position, end: Position, padding: i32) {
        let low = (start.row.min(start.col) - padding).unsigned_abs();
        let high = (end.row.max(end.col) + 1).max(0) as u32;
        let required = low.max(high);
        if required <= self.radius as u32 {
            return;
        }

        let next_radius = required.next_power_of_two() as i32;
        let next_side = 2 * next_radius as usize;
        let side = 2 * self.radius as usize;
        let shift = (next_radius - self.radius) as usize;
        let mut cells = vec![0; next_side * next_side];
        for (row, source) in self.cells.chunks_exact(side).enumerate() {
            let start = (row + shift) * next_side + shift;
            cells[start..start + side].copy_from_slice(source);
        }

        self.radius = next_radius;
        self.cells = cells;
        self.seen = vec![[0; 2]; next_side * next_side];
    }

    fn index(&self, position: Position) -> usize {
        (position.row + self.radius) as usize * (2 * self.radius as usize)
            + (position.col + self.radius) as usize
    }

    fn at(&self, position: Position) -> u16 {
        let row = (position.row + self.radius) as usize;
        let col = (position.col + self.radius) as usize;
        let side = 2 * self.radius as usize;
        if row < side && col < side {
            self.cells[row * side + col]
        } else {
            0
        }
    }

    fn start_round(&mut self) {
        self.round = self.round.wrapping_add(1);
        if self.round == 0 {
            self.seen.fill([0; 2]);
            self.round = 1;
        }
    }

    fn mark_new(&mut self, placement: Placement) -> bool {
        let index = self.index(placement.start);
        let slot = &mut self.seen[index][placement.direction as usize];
        let fresh = *slot != self.round;
        *slot = self.round;
        fresh
    }
}

struct Layout {
    board: Board,
    by_letter: [Vec<Position>; 256],
    top: i32,
    left: i32,
    bottom: i32,
    right: i32,
    padding: i32,
}

impl Layout {
    fn new(padding: i32) -> Self {
        Self {
            board: Board::new(),
            by_letter: std::array::from_fn(|_| Vec::new()),
            top: 0,
            left: 0,
            bottom: -1,
            right: -1,
            padding,
        }
    }

    fn add(&mut self, word: &Word, placement: Placement, output: &mut Vec<i32>) {
        let start = placement.start;
        let end = placement.position(word.letters.len() as i32 - 1);
        self.board.reserve(start, end, self.padding);

        for (offset, &letter) in word.letters.iter().enumerate() {
            let position = placement.position(offset as i32);
            let index = self.board.index(position);
            let cell = &mut self.board.cells[index];
            if *cell == 0 {
                *cell = u16::from(letter);
                self.by_letter[letter as usize].push(position);
            }
            *cell |= placement.direction.bit();
        }

        self.top = self.top.min(start.row);
        self.left = self.left.min(start.col);
        self.bottom = self.bottom.max(end.row);
        self.right = self.right.max(end.col);
        output.extend([word.index, start.row, start.col, placement.direction as i32]);
    }

    fn score(&self, word: &Word, placement: Placement) -> Option<f64> {
        let direction = placement.direction;
        let end = placement.position(word.letters.len() as i32 - 1);
        if self.board.at(placement.position(-1)) != 0
            || self.board.at(direction.offset(end, 1)) != 0
        {
            return None;
        }

        let mut crossings = 0;
        let side = direction.perpendicular();
        for (offset, &letter) in word.letters.iter().enumerate() {
            let position = placement.position(offset as i32);
            let cell = self.board.at(position);
            if cell != 0 {
                if cell as u8 != letter || cell & direction.bit() != 0 {
                    return None;
                }
                crossings += 1;
            } else if self.board.at(side.offset(position, -1)) != 0
                || self.board.at(side.offset(position, 1)) != 0
            {
                return None;
            }
        }

        if crossings == 0 {
            return None;
        }
        let rows = f64::from(self.bottom.max(end.row) - self.top.min(placement.start.row) + 1);
        let cols = f64::from(self.right.max(end.col) - self.left.min(placement.start.col) + 1);
        let longest = rows.max(cols);
        Some(rows * cols + longest * longest * 0.4 - f64::from(crossings) * 18.0)
    }

    fn find_crossing(&mut self, word: &Word, random: &mut Random) -> Option<Placement> {
        self.board.start_round();
        let mut best = None;
        let mut best_score = f64::INFINITY;

        for (offset, &letter) in word.letters.iter().enumerate() {
            for &position in &self.by_letter[letter as usize] {
                let occupied = self.board.at(position);
                for direction in [Direction::Across, Direction::Down] {
                    if occupied & direction.bit() != 0 {
                        continue;
                    }
                    let placement = Placement {
                        start: direction.offset(position, -(offset as i32)),
                        direction,
                    };
                    if !self.board.mark_new(placement) {
                        continue;
                    }
                    let Some(score) = self.score(word, placement) else {
                        continue;
                    };
                    let score = score + random.fraction();
                    if score < best_score {
                        best = Some(placement);
                        best_score = score;
                    }
                }
            }
        }
        best
    }

    fn disconnected(&self) -> Placement {
        // Keep a clear one-cell gap between independently placed words.
        if self.right - self.left <= self.bottom - self.top {
            Placement {
                start: Position {
                    row: self.top,
                    col: self.right + 2,
                },
                direction: Direction::Down,
            }
        } else {
            Placement {
                start: Position {
                    row: self.bottom + 2,
                    col: self.left,
                },
                direction: Direction::Across,
            }
        }
    }
}

fn create_layout(mut input: &[u32], seed: u32, output: &mut Vec<i32>) -> u32 {
    if input.is_empty() || input.len() > (i32::MAX / 4) as usize {
        return 0;
    }

    let mut random = Random(seed);
    let mut pending = Vec::new();
    while let Some((&length, rest)) = input.split_first() {
        if length == 0 || length as usize > rest.len() {
            return 0;
        }
        let (units, next) = rest.split_at(length as usize);
        let mut letters = Vec::with_capacity(units.len());
        for &unit in units {
            if !(0x0400..=0x04ff).contains(&unit) {
                return 0;
            }
            letters.push((unit - 0x0400) as u8);
        }
        pending.push(Word {
            index: pending.len() as i32,
            letters,
            tie: random.next(),
        });
        input = next;
    }
    pending.sort_by(|a, b| {
        b.letters
            .len()
            .cmp(&a.letters.len())
            .then(a.tie.cmp(&b.tie))
    });

    let count = pending.len();
    let first = pending.remove(0);
    let mut layout = Layout::new(first.letters.len() as i32 - 1);
    let direction = if random.fraction() < 0.5 {
        Direction::Across
    } else {
        Direction::Down
    };
    output.reserve(2 + count * 4);
    output.extend([0, 0]);
    layout.add(
        &first,
        Placement {
            start: Position { row: 0, col: 0 },
            direction,
        },
        output,
    );

    while !pending.is_empty() {
        let remaining = pending.len();
        let mut index = 0;
        while index < pending.len() {
            if let Some(placement) = layout.find_crossing(&pending[index], &mut random) {
                layout.add(&pending.remove(index), placement, output);
            } else {
                index += 1;
            }
        }
        if pending.len() == remaining {
            layout.add(&pending.remove(0), layout.disconnected(), output);
        }
    }

    output[0] = layout.bottom - layout.top + 1;
    output[1] = layout.right - layout.left + 1;
    for placement in output[2..].chunks_exact_mut(4) {
        placement[1] -= layout.top;
        placement[2] -= layout.left;
    }
    count as u32
}
