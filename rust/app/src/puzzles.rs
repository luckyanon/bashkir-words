mod classic;

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

const ALPHABET: &str = "АБВГҒДҘЕЁЖЗИЙКҠЛМНҢОӨПРСҪТУҮФХҺЦЧШЩЪЫЬЭӘЮЯ";
const GENERATOR_VERSION: u32 = 6;
const MAX_LEVEL: u64 = 9_007_199_254_740_991;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Classic,
    Hardcore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Across,
    Down,
}

impl Direction {
    fn step(self) -> (i32, i32) {
        match self {
            Direction::Across => (0, 1),
            Direction::Down => (1, 0),
        }
    }

    fn bit(self) -> u8 {
        match self {
            Direction::Across => 1,
            Direction::Down => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Placement {
    pub word: String,
    pub entry_id: String,
    pub row: i32,
    pub col: i32,
    pub direction: Direction,
}

impl Placement {
    pub fn cells(&self) -> impl Iterator<Item = (i32, i32, char)> + '_ {
        let (dr, dc) = self.direction.step();
        self.word.chars().enumerate().map(move |(offset, letter)| {
            let offset = offset as i32;
            (self.row + dr * offset, self.col + dc * offset, letter)
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub row: i32,
    pub col: i32,
    pub letter: char,
    pub word_indexes: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Level {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub mode: Mode,
    pub letters: Vec<char>,
    pub words: Vec<Placement>,
    pub bonus_words: Vec<String>,
    pub rows: i32,
    pub cols: i32,
    pub layout: String,
    pub difficulty_revision: u32,
    pub generator_version: u32,
    pub layout_version: u32,
    pub min_word_length: usize,
    pub excluded_word_lengths: Vec<usize>,
}

impl Level {
    pub fn cells(&self) -> Vec<Cell> {
        let mut cells: Vec<Cell> = Vec::new();
        let mut indexes: HashMap<(i32, i32), usize> = HashMap::new();
        for (index, target) in self.words.iter().enumerate() {
            for (row, col, letter) in target.cells() {
                if let Some(&previous) = indexes.get(&(row, col)) {
                    cells[previous].word_indexes.push(index);
                } else {
                    indexes.insert((row, col), cells.len());
                    cells.push(Cell {
                        row,
                        col,
                        letter,
                        word_indexes: vec![index],
                    });
                }
            }
        }
        cells.sort_by_key(|cell| (cell.row, cell.col));
        cells
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LevelOptions {
    pub layout_version: u32,
    pub layout_legacy_through: u64,
    pub include_local_words: bool,
    pub apply_level_difficulty: bool,
}

impl Default for LevelOptions {
    fn default() -> Self {
        Self {
            layout_version: 2,
            layout_legacy_through: 0,
            include_local_words: true,
            apply_level_difficulty: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Meaning {
    pub text: String,
    pub part_of_speech: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Definitions {
    #[serde(default)]
    pub ru: Vec<Meaning>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub url: Option<String>,
    pub article_url: Option<String>,
    pub page_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryEntry {
    pub id: String,
    pub word: String,
    #[serde(default)]
    pub display: String,
    #[serde(default)]
    pub parts_of_speech: Vec<String>,
    #[serde(default)]
    pub definitions: Definitions,
    #[serde(default)]
    pub sources: Vec<Source>,
}

#[derive(Deserialize)]
struct DetailedDictionary {
    entries: Vec<DictionaryEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Lexicon {
    schema_version: String,
    language: String,
    #[serde(default)]
    part_of_speech: String,
    min_length: usize,
    max_length: usize,
    words: Vec<String>,
    target_words: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Target {
    word: String,
    entry_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pool {
    letters: Vec<char>,
    words: Vec<Target>,
    bonus_words: Vec<String>,
    min_word_length: usize,
    excluded_word_lengths: Vec<usize>,
    #[serde(default)]
    hardcore_word_count: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PoolData {
    schema_version: String,
    dataset_revision: String,
    generator_version: u32,
    part_of_speech: String,
    max_word_length: usize,
    pools: Vec<Pool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Archive {
    raw: Lexicon,
    pools: PoolData,
    removed_entries: Vec<DictionaryEntry>,
}

struct SearchWord {
    word: String,
    counts: Vec<(usize, u16)>,
    target: bool,
}

pub struct PuzzleBook {
    pub dataset_revision: String,
    words: HashSet<String>,
    targets: HashSet<String>,
    entries: Vec<DictionaryEntry>,
    entries_by_spelling: HashMap<String, usize>,
    entries_by_id: HashMap<String, usize>,
    buckets: Vec<Vec<SearchWord>>,
    pools: Vec<Pool>,
    groups: [Vec<usize>; 13],
    hardcore_indexes: Vec<usize>,
    max_word_length: usize,
    has_local_bal: bool,
}

fn trim(value: &str) -> &str {
    value.trim_matches(|c: char| matches!(c, '\u{feff}') || (c.is_whitespace() && c != '\u{85}'))
}

pub fn normalize_word(value: &str) -> String {
    let unaccented: String = value
        .nfc()
        .filter(|character| !matches!(character, '\u{0301}' | '\u{0341}' | '\u{00ad}'))
        .nfc()
        .collect();
    trim(&unaccented).to_uppercase()
}

fn letter_index(letter: char) -> Option<usize> {
    ALPHABET
        .contains(letter)
        .then_some((letter as usize).wrapping_sub(0x0400))
}

fn rack_counts(rack: &[char]) -> Option<[u16; 256]> {
    if rack.is_empty() {
        return None;
    }
    let mut available = [0u16; 256];
    for &letter in rack {
        let index = letter_index(letter)?;
        available[index] = available[index].saturating_add(1);
    }
    Some(available)
}

fn fits(word: &str, rack: &[char]) -> bool {
    let Some(mut available) = rack_counts(rack) else {
        return false;
    };
    for letter in word.chars() {
        let Some(index) = letter_index(letter) else {
            return false;
        };
        let Some(remaining) = available[index].checked_sub(1) else {
            return false;
        };
        available[index] = remaining;
    }
    !word.is_empty()
}

pub fn can_build_word(word: &str, rack: &[char]) -> bool {
    let rack = rack
        .iter()
        .copied()
        .map(|letter| {
            let normalized = normalize_word(&letter.to_string());
            let mut letters = normalized.chars();
            letters.next().filter(|_| letters.next().is_none())
        })
        .collect::<Option<Vec<_>>>();
    rack.is_some_and(|rack| fits(&normalize_word(word), &rack))
}

pub(crate) fn signature(rack: &str) -> String {
    let mut letters: Vec<_> = rack.chars().collect();
    letters.sort_by_key(|&letter| ALPHABET.find(letter));
    letters.into_iter().collect()
}

fn min_word_length_for_rack(length: usize) -> usize {
    match length {
        0..=5 => 2,
        6 => 3,
        _ => 4,
    }
}

fn classic_max_length(id: u64, maximum: usize) -> usize {
    maximum.min(match id {
        0..20 => 6,
        20..40 => 7,
        40..60 => 8,
        60..80 => 9,
        80..100 => 10,
        _ => maximum,
    })
}

fn hardcore_max_length(id: u64, maximum: usize) -> usize {
    maximum.min(match id {
        0..20 => 8,
        20..50 => 9,
        50..70 => 10,
        70..100 => 11,
        _ => 12,
    })
}

fn hash_seed(seed: &str) -> u32 {
    seed.chars().fold(2166136261u32, |state, c| {
        (state ^ c as u32).wrapping_mul(16777619)
    })
}

struct Random(u32);

impl Random {
    fn new(seed: &str) -> Self {
        Self(hash_seed(seed))
    }

    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6D2B79F5);
        let mut next = self.0;
        next = (next ^ (next >> 15)).wrapping_mul(next | 1);
        next ^= next.wrapping_add((next ^ (next >> 7)).wrapping_mul(next | 61));
        f64::from(next ^ (next >> 14)) / 4294967296.0
    }

    fn index(&mut self, length: usize) -> usize {
        (self.next() * length as f64).floor() as usize
    }

    fn shuffle<T>(&mut self, data: &mut [T]) {
        for index in (1..data.len()).rev() {
            data.swap(index, self.index(index + 1));
        }
    }
}

impl PuzzleBook {
    pub fn from_json(lexicon: &str, full_dict: &str, pools: &str) -> Result<Self, String> {
        let lexicon: Lexicon =
            serde_json::from_str(lexicon).map_err(|err| format!("Словарь: {err}"))?;
        let detailed: DetailedDictionary =
            serde_json::from_str(full_dict).map_err(|err| format!("Толкования: {err}"))?;
        let pools: PoolData =
            serde_json::from_str(pools).map_err(|err| format!("Уровни: {err}"))?;
        Self::from_parts(lexicon, detailed.entries, pools)
    }

    pub fn from_archive(current: &Self, archive: &str) -> Result<Self, String> {
        let archive: Archive =
            serde_json::from_str(archive).map_err(|err| format!("Архив: {err}"))?;
        let mut entries = current.entries.clone();
        entries.extend(archive.removed_entries);
        Self::from_parts(archive.raw, entries, archive.pools)
    }

    fn from_parts(
        lexicon: Lexicon,
        entries: Vec<DictionaryEntry>,
        pools: PoolData,
    ) -> Result<Self, String> {
        if lexicon.schema_version != "1.0.0"
            || lexicon.language != "ba"
            || lexicon.min_length == 0
            || lexicon.min_length > lexicon.max_length
            || lexicon.max_length > 12
        {
            return Err("Неподдерживаемый формат словаря".to_owned());
        }
        if pools.schema_version != "6.0.0"
            || pools.generator_version != GENERATOR_VERSION
            || pools.part_of_speech != "noun"
            || !(9..=12).contains(&pools.max_word_length)
            || pools.pools.is_empty()
        {
            return Err("Неподдерживаемый формат уровней".to_owned());
        }

        let words: HashSet<_> = lexicon.words.iter().cloned().collect();
        let targets: HashSet<_> = lexicon.target_words.into_iter().collect();
        if !targets.is_subset(&words) {
            return Err("Целевые слова отсутствуют в словаре".to_owned());
        }
        let mut buckets: Vec<Vec<SearchWord>> =
            (0..=lexicon.max_length).map(|_| Vec::new()).collect();
        let mut seen = HashSet::new();
        for word in &lexicon.words {
            if !seen.insert(word) {
                continue;
            }
            let letters: Vec<_> = word.chars().collect();
            let length = letters.len();
            if !(lexicon.min_length..=lexicon.max_length).contains(&length)
                || normalize_word(word) != *word
            {
                return Err(format!("Некорректное словарное слово: {word}"));
            }
            let counts =
                rack_counts(&letters).ok_or_else(|| format!("Некорректные буквы: {word}"))?;
            let counts = counts
                .into_iter()
                .enumerate()
                .filter(|(_, count)| *count != 0)
                .collect();
            buckets[length].push(SearchWord {
                word: word.clone(),
                counts,
                target: targets.contains(word),
            });
        }

        let mut entries_by_spelling = HashMap::new();
        let mut entries_by_id = HashMap::new();
        for (index, entry) in entries.iter().enumerate() {
            entries_by_id.insert(entry.id.clone(), index);
            for value in [&entry.display, &entry.word] {
                let normalized = normalize_word(value);
                if !normalized.is_empty() {
                    entries_by_spelling.insert(normalized, index);
                }
            }
        }
        let groups = std::array::from_fn(|_| Vec::new());
        let has_local_bal =
            lexicon.part_of_speech == "noun" && entries_by_spelling.contains_key("БАЛ");
        let mut book = Self {
            dataset_revision: pools.dataset_revision,
            words,
            targets,
            entries,
            entries_by_spelling,
            entries_by_id,
            buckets,
            pools: pools.pools,
            groups,
            hardcore_indexes: Vec::new(),
            max_word_length: pools.max_word_length,
            has_local_bal,
        };
        for (index, pool) in book.pools.iter().enumerate() {
            book.validate_pool(pool)?;
            book.groups[pool.letters.len()].push(index);
            if pool.hardcore_word_count >= 30 {
                book.hardcore_indexes.push(index);
            }
        }
        Ok(book)
    }

    fn validate_pool(&self, pool: &Pool) -> Result<(), String> {
        let length = pool.letters.len();
        let min = min_word_length_for_rack(length);
        let rack: String = pool.letters.iter().collect();
        if !(3..=self.max_word_length).contains(&length)
            || !(8..=12).contains(&pool.words.len())
            || pool.bonus_words.len() > 3
            || pool.min_word_length != min
            || pool.excluded_word_lengths.iter().copied().ne(2..min)
            || signature(&rack) != rack
            || !pool.words.iter().any(|target| {
                target.word.chars().count() == length && signature(&target.word) == rack
            })
        {
            return Err(format!("Некорректный набор букв: {rack}"));
        }
        let mut declared = HashSet::new();
        for word in pool
            .words
            .iter()
            .map(|target| &target.word)
            .chain(&pool.bonus_words)
        {
            if !declared.insert(word)
                || word.chars().count() < min
                || !fits(word, &pool.letters)
                || !self.words.contains(word)
            {
                return Err(format!("Некорректное слово в наборе {rack}: {word}"));
            }
        }
        for target in &pool.words {
            let entry = self
                .entries_by_spelling
                .get(&target.word)
                .map(|&index| &self.entries[index]);
            let valid = self.targets.contains(&target.word)
                && entry.is_some_and(|entry| {
                    !target.entry_id.is_empty()
                        && entry.id == target.entry_id
                        && entry.parts_of_speech.iter().any(|pos| pos == "noun")
                        && normalize_word(if entry.display.is_empty() {
                            &entry.word
                        } else {
                            &entry.display
                        }) == target.word
                        && entry
                            .definitions
                            .ru
                            .iter()
                            .any(|sense| !trim(&sense.text).is_empty())
                });
            if !valid {
                return Err(format!("Отсутствует исходная статья: {}", target.word));
            }
        }
        Ok(())
    }

    pub fn has_word(&self, word: &str) -> bool {
        self.words.contains(&normalize_word(word))
    }

    pub fn can_build(&self, word: &str, rack: &[char]) -> bool {
        let word = normalize_word(word);
        self.words.contains(&word) && fits(&word, rack)
    }

    pub fn entry(&self, word: &str) -> Option<&DictionaryEntry> {
        self.entries_by_spelling
            .get(&normalize_word(word))
            .map(|&i| &self.entries[i])
    }

    pub fn entry_by_id(&self, id: &str) -> Option<&DictionaryEntry> {
        self.entries_by_id.get(id).map(|&i| &self.entries[i])
    }

    pub fn meanings(&self, word: &str) -> Vec<&str> {
        self.entry(word)
            .into_iter()
            .flat_map(|entry| &entry.definitions.ru)
            .map(|sense| sense.text.as_str())
            .filter(|text| !trim(text).is_empty())
            .collect()
    }

    pub fn is_target_candidate(&self, word: &str) -> bool {
        self.targets.contains(&normalize_word(word)) && !self.meanings(word).is_empty()
    }

    pub fn find_words(&self, rack: &[char], min_length: usize, targets_only: bool) -> Vec<String> {
        let Some(available) = rack_counts(rack) else {
            return Vec::new();
        };
        self.buckets
            .iter()
            .take(rack.len() + 1)
            .skip(min_length)
            .flatten()
            .filter(|entry| {
                (!targets_only || entry.target)
                    && entry
                        .counts
                        .iter()
                        .all(|&(index, count)| available[index] >= count)
            })
            .map(|entry| entry.word.clone())
            .collect()
    }

    pub fn level(
        &self,
        id: u64,
        seed: &str,
        mode: Mode,
        options: LevelOptions,
    ) -> Result<Level, String> {
        if !(1..=MAX_LEVEL).contains(&id) || !matches!(options.layout_version, 1 | 2) {
            return Err("Некорректный номер или формат уровня".to_owned());
        }
        let difficulty_revision = if options.apply_level_difficulty { 2 } else { 0 };
        if mode == Mode::Hardcore {
            return self.hardcore_level(id, seed, difficulty_revision);
        }
        let (min_size, max_size) = if options.apply_level_difficulty {
            (
                if id < 80 { 3 } else { 4 },
                classic_max_length(id, self.max_word_length),
            )
        } else {
            (3, self.max_word_length)
        };
        let mut sizes = (min_size..=max_size)
            .filter(|&size| !self.groups[size].is_empty())
            .collect::<Vec<_>>();
        let size_count = sizes.len() as u64;
        if size_count == 0 {
            return Err("Нет подходящих наборов букв".to_owned());
        }
        let size_key = sizes
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let block = (id - 1) / size_count;
        let offset = ((id - 1) % size_count) as usize;
        Random::new(&format!("{seed}:sizes:2:{size_key}:{block}")).shuffle(&mut sizes);
        let category = sizes[offset];
        let mut order = self.groups[category].clone();
        let order_count = order.len() as u64;
        let cycle = block / order_count;
        Random::new(&format!("{seed}:pools:2:{category}:{cycle}")).shuffle(&mut order);
        let pool = &self.pools[order[(block % order_count) as usize]];
        let layout_version = if options.layout_version == 1 || id <= options.layout_legacy_through {
            1
        } else {
            2
        };
        let local_bal = if options.include_local_words
            && self.has_local_bal
            && pool.min_word_length <= 3
            && self.can_build("БАЛ", &pool.letters)
            && !pool.words.iter().any(|entry| entry.word == "БАЛ")
        {
            self.entry("БАЛ").map(|entry| Target {
                word: "БАЛ".to_owned(),
                entry_id: entry.id.clone(),
            })
        } else {
            None
        };
        let (layout, letters) = classic::generate(
            &pool.words,
            &pool.letters,
            seed,
            id,
            layout_version,
            local_bal,
        )?;
        Ok(Level {
            id,
            title: "Башкирские слова".to_owned(),
            mode,
            letters,
            words: layout.words,
            bonus_words: pool.bonus_words.clone(),
            rows: layout.rows,
            cols: layout.cols,
            layout: "free".to_owned(),
            difficulty_revision,
            generator_version: GENERATOR_VERSION,
            layout_version,
            min_word_length: pool.min_word_length,
            excluded_word_lengths: pool.excluded_word_lengths.clone(),
        })
    }

    fn hardcore_level(
        &self,
        id: u64,
        seed: &str,
        difficulty_revision: u32,
    ) -> Result<Level, String> {
        let cap = hardcore_max_length(id, self.max_word_length);
        let mut indexes: Vec<_> = self
            .hardcore_indexes
            .iter()
            .copied()
            .filter(|&index| self.pools[index].letters.len() <= cap)
            .collect();
        let count = indexes.len() as u64;
        if count == 0 {
            return Err("Нет доступных наборов для хардкора".to_owned());
        }
        let cycle = (id - 1) / count;
        Random::new(&format!("{seed}:hardcore:2:pools:{cap}:{cycle}")).shuffle(&mut indexes);
        let pool = &self.pools[indexes[((id - 1) % count) as usize]];
        let words = self.find_words(&pool.letters, 2, true);
        if words.len() != pool.hardcore_word_count || words.len() < 30 {
            return Err("Устаревший набор слов хардкора".to_owned());
        }
        let views = words.iter().map(String::as_str).collect::<Vec<_>>();
        let layout = bashkir_hardcore::layout_for_words(
            &views,
            hash_seed(&format!("{seed}:hardcore:2:{id}")),
        )
        .ok_or_else(|| "Не удалось построить поле хардкора".to_string())?;
        let words = layout
            .placements
            .into_iter()
            .map(|placement| {
                let word = &words[placement.word_index];
                let entry = self
                    .entry(word)
                    .ok_or_else(|| format!("Отсутствует статья: {word}"))?;
                let direction = if placement.down {
                    Direction::Down
                } else {
                    Direction::Across
                };
                Ok(Placement {
                    word: word.clone(),
                    entry_id: entry.id.clone(),
                    row: placement.row,
                    col: placement.col,
                    direction,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let bonus_words = self
            .find_words(&pool.letters, 2, false)
            .into_iter()
            .filter(|word| !self.is_target_candidate(word))
            .take(3)
            .collect();
        let mut letters = pool.letters.clone();
        Random::new(&format!("{seed}:hardcore:rack:{id}")).shuffle(&mut letters);
        Ok(Level {
            id,
            title: "Хардкор".to_owned(),
            mode: Mode::Hardcore,
            letters,
            words,
            bonus_words,
            rows: layout.rows,
            cols: layout.cols,
            layout: "free".to_owned(),
            difficulty_revision,
            generator_version: GENERATOR_VERSION,
            layout_version: 2,
            min_word_length: 2,
            excluded_word_lengths: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;

    use super::*;

    fn layout_hash(level: &Level) -> u32 {
        let letters: String = level.letters.iter().collect();
        let mut layout = format!("{}x{}:{letters}", level.rows, level.cols);
        for word in &level.words {
            let direction = match word.direction {
                Direction::Across => "across",
                Direction::Down => "down",
            };
            write!(
                layout,
                "|{}:{}:{}:{}:{direction}",
                word.word, word.entry_id, word.row, word.col
            )
            .unwrap();
        }
        hash_seed(&layout)
    }

    #[test]
    fn saved_levels_match_javascript_2_10_5_layouts() {
        let book = PuzzleBook::from_json(
            include_str!("../../../public/data/game-lexicon.nouns.json"),
            include_str!("../../../public/data/nouns.full.json"),
            include_str!("../../../public/data/level-pools.json"),
        )
        .unwrap();
        let current = LevelOptions::default();
        let legacy = LevelOptions {
            layout_legacy_through: 1,
            ..current
        };
        let fixtures = [
            (1, "Һаумы😀", Mode::Classic, current, 2_287_069_218),
            (1, "Һаумы😀", Mode::Classic, legacy, 967_132_411),
            (
                100,
                "Һаумы😀:hardcore",
                Mode::Hardcore,
                current,
                999_962_351,
            ),
        ];
        for (id, seed, mode, options, expected) in fixtures {
            let level = book.level(id, seed, mode, options).unwrap();
            assert_eq!(
                layout_hash(&level),
                expected,
                "{mode:?}, level {id}, {options:?}"
            );
        }
    }
}
