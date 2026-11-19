use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};

use super::{Direction, Placement, Random, Target};

const MAX_SPAN: i32 = 24;
type Point = (i32, i32);

#[derive(Clone, Copy)]
struct LetterCell {
    point: Point,
    letter: char,
    directions: u8,
}

#[derive(Default)]
struct Board {
    ordered: Vec<LetterCell>,
    lookup: HashMap<Point, usize>,
}

impl Board {
    fn from_words(words: &[Placement]) -> Self {
        let mut board = Self::default();
        for word in words {
            for (row, col, letter) in word.cells() {
                let point = (row, col);
                if let Some(&index) = board.lookup.get(&point) {
                    board.ordered[index].directions |= word.direction.bit();
                } else {
                    board.lookup.insert(point, board.ordered.len());
                    board.ordered.push(LetterCell {
                        point,
                        letter,
                        directions: word.direction.bit(),
                    });
                }
            }
        }
        board
    }

    fn get(&self, point: Point) -> Option<LetterCell> {
        self.lookup.get(&point).map(|&index| self.ordered[index])
    }

    fn contains(&self, point: Point) -> bool {
        self.lookup.contains_key(&point)
    }

    fn bounds(&self) -> Bounds {
        let mut bounds = Bounds::new(self.ordered[0].point);
        for cell in &self.ordered[1..] {
            bounds.extend(cell.point);
        }
        bounds
    }
}

#[derive(Clone, Copy)]
struct Bounds {
    top: i32,
    left: i32,
    bottom: i32,
    right: i32,
}

impl Bounds {
    fn new((row, col): Point) -> Self {
        Self {
            top: row,
            left: col,
            bottom: row,
            right: col,
        }
    }

    fn extend(&mut self, (row, col): Point) {
        self.top = self.top.min(row);
        self.left = self.left.min(col);
        self.bottom = self.bottom.max(row);
        self.right = self.right.max(col);
    }

    fn rows(self) -> i32 {
        self.bottom - self.top + 1
    }

    fn cols(self) -> i32 {
        self.right - self.left + 1
    }

    fn longest(self) -> i32 {
        self.rows().max(self.cols())
    }
}

#[derive(Debug)]
pub(super) struct Layout {
    pub rows: i32,
    pub cols: i32,
    pub words: Vec<Placement>,
}

fn normalize(mut words: Vec<Placement>) -> Layout {
    let bounds = Board::from_words(&words).bounds();
    for word in &mut words {
        word.row -= bounds.top;
        word.col -= bounds.left;
    }
    Layout {
        rows: bounds.rows(),
        cols: bounds.cols(),
        words,
    }
}

fn offset((row, col): Point, (dr, dc): Point, steps: i32) -> Point {
    (row + steps * dr, col + steps * dc)
}

fn crossing(
    letters: &[char],
    start: Point,
    direction: Direction,
    board: &Board,
    current: Bounds,
    compact: bool,
) -> Option<(Bounds, i32)> {
    let (dr, dc) = direction.step();
    let end = offset(start, (dr, dc), letters.len() as i32 - 1);
    if board.contains(offset(start, (dr, dc), -1)) || board.contains(offset(end, (dr, dc), 1)) {
        return None;
    }
    let mut intersections = 0;
    for (index, &letter) in letters.iter().enumerate() {
        let point = offset(start, (dr, dc), index as i32);
        if let Some(cell) = board.get(point) {
            if cell.letter != letter || cell.directions & direction.bit() != 0 {
                return None;
            }
            intersections += 1;
        } else if board.contains(offset(point, (dc, dr), -1))
            || board.contains(offset(point, (dc, dr), 1))
        {
            return None;
        }
    }
    let mut bounds = current;
    bounds.extend(start);
    bounds.extend(end);
    if compact && intersections > 0 && bounds.longest() <= MAX_SPAN
        || !compact && intersections == 1 && bounds.longest() <= 10
    {
        Some((bounds, intersections))
    } else {
        None
    }
}

fn shape_score(rows: i32, cols: i32, weight: f64) -> f64 {
    f64::from(rows * cols) + f64::from(rows.max(cols).pow(2)) * weight
}

fn placement(entry: &Target, (row, col): Point, direction: Direction) -> Placement {
    Placement {
        word: entry.word.clone(),
        entry_id: entry.entry_id.clone(),
        row,
        col,
        direction,
    }
}

fn clusters(source: &[Target], random: &mut Random, compact: bool) -> Vec<Layout> {
    let mut pending = source.iter().collect::<Vec<_>>();
    random.shuffle(&mut pending);
    if compact {
        pending.sort_by_key(|entry| Reverse(entry.word.chars().count()));
    }
    let mut result = Vec::new();
    while !pending.is_empty() {
        let direction = if random.next() < 0.5 {
            Direction::Across
        } else {
            Direction::Down
        };
        let mut words = vec![placement(pending.remove(0), (0, 0), direction)];
        let limit = if compact {
            usize::MAX
        } else {
            2 + random.index(3)
        };
        while !pending.is_empty() && words.len() < limit {
            let board = Board::from_words(&words);
            let current = board.bounds();
            let mut choices = Vec::new();
            let mut best_score = f64::INFINITY;
            let mut best = None;
            for (index, entry) in pending.iter().enumerate() {
                let letters = entry.word.chars().collect::<Vec<_>>();
                let mut seen = HashSet::new();
                for cell in &board.ordered {
                    for (letter_index, &letter) in letters.iter().enumerate() {
                        if cell.letter != letter {
                            continue;
                        }
                        for direction in [Direction::Across, Direction::Down] {
                            if cell.directions & direction.bit() != 0 {
                                continue;
                            }
                            let start =
                                offset(cell.point, direction.step(), -(letter_index as i32));
                            if !seen.insert((start, direction)) {
                                continue;
                            }
                            let Some((bounds, intersections)) =
                                crossing(&letters, start, direction, &board, current, compact)
                            else {
                                continue;
                            };
                            let candidate = (index, start, direction);
                            if compact {
                                let score = shape_score(bounds.rows(), bounds.cols(), 0.3)
                                    - f64::from(intersections * 25)
                                    + random.next() * 3.0;
                                if score < best_score {
                                    best_score = score;
                                    best = Some(candidate);
                                }
                            } else {
                                choices.push(candidate);
                            }
                        }
                    }
                }
            }
            if !compact && !choices.is_empty() {
                best = Some(choices[random.index(choices.len())]);
            }
            let Some((index, start, direction)) = best else {
                break;
            };
            words.push(placement(pending.remove(index), start, direction));
        }
        result.push(normalize(words));
    }
    random.shuffle(&mut result);
    result
}

fn detached_fits(board: &Board, cluster: &Board, (row, col): Point) -> bool {
    cluster.ordered.iter().all(|cell| {
        let (cr, cc) = cell.point;
        let (r, c) = (row + cr, col + cc);
        [(r, c), (r - 1, c), (r + 1, c), (r, c - 1), (r, c + 1)]
            .into_iter()
            .all(|position| !board.contains(position))
    })
}

fn pack(clusters: Vec<Layout>, random: &mut Random) -> Option<Layout> {
    let mut clusters = clusters.into_iter();
    let mut words = clusters.next()?.words;
    for cluster in clusters {
        let board = Board::from_words(&words);
        let current = board.bounds();
        let other = Board::from_words(&cluster.words);
        let preferred_edge = random.index(4);
        let mut best = None;
        let mut best_score = f64::INFINITY;
        for row in current.top - cluster.rows - 2..=current.bottom + 2 {
            for col in current.left - cluster.cols - 2..=current.right + 2 {
                if !detached_fits(&board, &other, (row, col)) {
                    continue;
                }
                let mut bounds = current;
                bounds.extend((row, col));
                bounds.extend((row + cluster.rows - 1, col + cluster.cols - 1));
                if bounds.longest() > MAX_SPAN {
                    continue;
                }
                let edge = if row + cluster.rows <= current.top {
                    0
                } else if col > current.right {
                    1
                } else if row > current.bottom {
                    2
                } else {
                    3
                };
                let edge_preference = if edge == preferred_edge { -18.0 } else { 0.0 };
                let score = shape_score(bounds.rows(), bounds.cols(), 0.65)
                    + f64::from((bounds.rows() - bounds.cols()).abs() * 3)
                    + edge_preference
                    + random.next() * 22.0;
                if score < best_score {
                    best_score = score;
                    best = Some((row, col));
                }
            }
        }
        let (row, col) = best?;
        words.extend(cluster.words.into_iter().map(|mut word| {
            word.row += row;
            word.col += col;
            word
        }));
    }
    Some(normalize(words))
}

fn compact_layout(source: &[Target], seed: &str, mut best: Layout, components: usize) -> Layout {
    let score = |layout: &Layout, components| {
        (components * 10000 + (layout.rows * layout.cols) as usize) as f64
            + f64::from(layout.rows.max(layout.cols).pow(2)) * 0.3
    };
    let mut best_score = score(&best, components);
    for attempt in 0..8 {
        let mut random = Random::new(&format!("{seed}:{attempt}"));
        let groups = clusters(source, &mut random, true);
        let components = groups.len();
        let Some(candidate) = pack(groups, &mut random) else {
            continue;
        };
        let candidate_score = score(&candidate, components);
        if candidate_score < best_score {
            best = candidate;
            best_score = candidate_score;
        }
    }
    best
}

pub(super) fn generate(
    targets: &[Target],
    rack: &[char],
    campaign: &str,
    id: u64,
    version: u32,
    local_word: Option<Target>,
) -> Result<(Layout, Vec<char>), String> {
    let mut random = Random::new(&format!("6:{campaign}:{id}"));
    let groups = clusters(targets, &mut random, false);
    let components = groups.len();
    let fallback = pack(groups, &mut random)
        .ok_or_else(|| "Не удалось разместить слова уровня".to_string())?;
    let mut letters = rack.to_vec();
    random.shuffle(&mut letters);

    let mut layout = if version == 1 {
        fallback
    } else {
        compact_layout(
            targets,
            &format!("6:{campaign}:{id}:layout:2"),
            fallback,
            components,
        )
    };
    if let Some(added) = local_word {
        let full_rack = |entry: &&Target| entry.word.chars().count() == rack.len();
        let mut words = targets
            .iter()
            .filter(full_rack)
            .cloned()
            .collect::<Vec<_>>();
        words.push(added);
        words.extend(targets.iter().filter(|entry| !full_rack(entry)).cloned());
        words.truncate(12);
        let groups = clusters(
            &words,
            &mut Random::new(&format!("{campaign}:{id}:local-fallback")),
            true,
        );
        let fallback = pack(
            groups,
            &mut Random::new(&format!("{campaign}:{id}:local-pack")),
        )
        .ok_or_else(|| "Не удалось разместить слова уровня".to_string())?;
        layout = compact_layout(
            &words,
            &format!("6:{campaign}:{id}:local-words:1"),
            fallback,
            words.len(),
        );
    }
    Ok((layout, letters))
}
