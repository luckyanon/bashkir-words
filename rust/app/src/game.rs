pub use crate::puzzles::Mode;
use crate::puzzles::{Level, LevelOptions, PuzzleBook, can_build_word, normalize_word};
use crate::save::{self, Backgrounds, DailyGift, Hardcore, MAX_NUMBER, Progress, Save};
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

const MAX_LEVEL: u64 = MAX_NUMBER - 1;
const RECENT_LEVELS: usize = 64;
const CLASSIC_REWARD: u64 = 60;
const BONUS_REWARD: u64 = 5;
pub const HINT_COST: u64 = 20;
pub const GIFT_REWARD: u64 = 100;

pub struct Background {
    pub id: &'static str,
    pub title: &'static str,
    pub price: u64,
    pub legacy_level: u64,
}

pub const BACKGROUNDS: [Background; 5] = [
    Background {
        id: "ufa",
        title: "Уфа",
        price: 0,
        legacy_level: 1,
    },
    Background {
        id: "salavat",
        title: "Салават",
        price: 1_000,
        legacy_level: 20,
    },
    Background {
        id: "baymak",
        title: "Баймак",
        price: 1_600,
        legacy_level: 40,
    },
    Background {
        id: "village",
        title: "Деревня",
        price: 2_400,
        legacy_level: 60,
    },
    Background {
        id: "olasay",
        title: "У бабушки",
        price: 3_000,
        legacy_level: 80,
    },
];

#[derive(Clone, Debug)]
pub struct Cell {
    pub row: i32,
    pub col: i32,
    pub key: String,
    pub letter: char,
    pub words: Vec<String>,
}

pub struct Meaning {
    pub text: String,
    pub part_of_speech: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionResult {
    pub kind: &'static str,
    pub word: String,
    pub reward: u64,
    pub completed: bool,
}

impl ActionResult {
    fn new(kind: &'static str) -> Self {
        Self {
            kind,
            word: String::new(),
            reward: 0,
            completed: false,
        }
    }
}

struct ActiveLevel {
    level: Level,
    cells: Vec<Cell>,
    bonuses: Vec<Option<String>>,
}

impl ActiveLevel {
    fn new(level: Level, book: &PuzzleBook, legacy_bonus: Option<Vec<Option<String>>>) -> Self {
        let cells = level
            .cells()
            .into_iter()
            .map(|cell| Cell {
                row: cell.row,
                col: cell.col,
                key: format!("{},{}", cell.row, cell.col),
                letter: cell.letter,
                words: cell
                    .word_indexes
                    .into_iter()
                    .map(|index| level.words[index].word.clone())
                    .collect(),
            })
            .collect();
        let current = bonus_words(&level, book);
        let bonuses = legacy_bonus.map_or_else(
            || current.clone(),
            |previous| align_bonus(&previous, &current),
        );
        Self {
            level,
            cells,
            bonuses,
        }
    }
}

pub(crate) fn bonus_words(level: &Level, book: &PuzzleBook) -> Vec<Option<String>> {
    let mut words: Vec<_> = level.bonus_words.iter().cloned().map(Some).collect();
    if level.mode == Mode::Classic {
        let extras = book.find_words(&level.letters, level.min_word_length, true);
        words.resize(words.len().max(3), None);
        words.extend(
            extras
                .into_iter()
                .filter(|word| {
                    book.is_target_candidate(word)
                        && !level.words.iter().any(|target| target.word == *word)
                        && !level.bonus_words.contains(word)
                })
                .map(Some),
        );
    }
    words
}

pub(crate) fn align_bonus(
    previous: &[Option<String>],
    current: &[Option<String>],
) -> Vec<Option<String>> {
    previous
        .iter()
        .map(|word| word.as_ref().filter(|_| current.contains(word)).cloned())
        .chain(
            current
                .iter()
                .filter(|word| word.is_some() && !previous.contains(word))
                .cloned(),
        )
        .collect()
}

#[derive(Default)]
pub struct Effects {
    pub sound: Option<&'static str>,
    pub haptic: bool,
    pub request_notifications: bool,
}

pub struct Game {
    pub state: Save,
    pub rack: Vec<char>,
    pub local_day: i64,
    pub now_millis: i64,
    book: Arc<PuzzleBook>,
    history: crate::history::History,
    active: ActiveLevel,
    random: u64,
    recent: VecDeque<u64>,
    dirty: bool,
    effects: Effects,
}

impl Game {
    #[cfg(not(target_os = "android"))]
    pub fn new(
        book: Arc<PuzzleBook>,
        raw_save: &str,
        now_millis: i64,
        local_day: i64,
    ) -> Result<Self, String> {
        Self::new_with_history(book, raw_save, now_millis, local_day, |path| {
            std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../public")
                    .join(path),
            )
            .map_err(|error| format!("{path}: {error}"))
        })
    }

    pub fn new_with_history(
        book: Arc<PuzzleBook>,
        raw_save: &str,
        now_millis: i64,
        local_day: i64,
        load_archive: impl FnMut(&str) -> Result<String, String>,
    ) -> Result<Self, String> {
        let fallback_seed = format!("{now_millis}-{}", campaign_entropy(now_millis));
        let mut state = save::decode(raw_save)
            .unwrap_or_else(|| Save::new(fallback_seed.clone(), &book.dataset_revision));
        let missing_seed = state.campaign_seed.is_empty() || state.campaign_seed.len() > 256;
        let fallback_coordinates = state.version != 2 || state.generator_version != 6;
        if missing_seed {
            state.campaign_seed = fallback_seed;
        }
        state.current_level = state.current_level.clamp(1, MAX_LEVEL);
        state.hardcore.current_level = state.hardcore.current_level.clamp(1, MAX_LEVEL);
        save::normalize(&mut state.completed_ranges, MAX_LEVEL);
        if state.version == 1 {
            let unlocked = state
                .extra
                .get("unlocked")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_u64)
                .filter(|&id| (1..=32).contains(&id))
                .collect::<HashSet<_>>();
            let through = (2..=32)
                .take_while(|id| unlocked.contains(id))
                .last()
                .unwrap_or(1);
            if through > 1 {
                state.completed_ranges = vec![[1, through - 1]];
            }
            state.economy.base_coins = state.coins;
            state.economy.completed_baseline = save::count(&state.completed_ranges);
            state.extra.insert(
                "legacy".into(),
                serde_json::json!({
                    "version": 1, "currentLevel": state.current_level, "coins": state.coins,
                    "unlocked": unlocked, "levels": state.levels,
                }),
            );
            state.extra.insert(
                "migration".into(),
                serde_json::json!({"fromVersion":1, "boardReset":true}),
            );
        }
        state.max_unlocked = unlocked_level(&state.completed_ranges);
        state.current_level = state.current_level.min(state.max_unlocked);
        state.hint_count = state.hint_count.min(MAX_NUMBER);
        state.legacy_layout_through_level = state.legacy_layout_through_level.min(MAX_LEVEL);
        if state.layout_version <= 1 {
            state.legacy_layout_through_level =
                state.legacy_layout_through_level.max(state.max_unlocked);
        }
        state.bonus_ranges.truncate(12_976);
        state
            .bonus_ranges
            .resize_with(state.bonus_ranges.len().max(3), Vec::new);
        for ranges in &mut state.bonus_ranges {
            save::normalize(ranges, state.max_unlocked);
        }
        state
            .levels
            .retain(|id, _| *id > 0 && *id <= state.max_unlocked);
        while state.levels.len() > RECENT_LEVELS {
            if let Some(key) = state
                .levels
                .keys()
                .find(|&&key| key != state.current_level)
                .copied()
            {
                state.levels.remove(&key);
            }
        }
        normalize_backgrounds(&mut state);
        if let Some(last) = state
            .daily_gift
            .last_claim_at
            .take()
            .filter(|_| state.daily_gift.last_claim_day.is_none())
        {
            let local_offset = local_day - now_millis.div_euclid(86_400_000);
            state.daily_gift.last_claim_day = Some(last.div_euclid(86_400_000) + local_offset);
        }
        if state.daily_gift.count == 0
            || state.daily_gift.count > 100_000_001
            || !state
                .daily_gift
                .last_claim_day
                .is_some_and(|day| day >= state.daily_gift.count as i64 - 1 && day <= 100_000_000)
        {
            state.daily_gift = DailyGift::default();
        }
        normalize_hardcore(&mut state.hardcore);

        let before_history_revision = state.dataset_revision.clone();
        let history = crate::history::History::restore(&mut state, &book, load_archive)?;
        if fallback_coordinates || state.dataset_revision != book.dataset_revision || missing_seed {
            let options = level_options(&state);
            let seed = &state.campaign_seed;
            for (&id, progress) in &mut state.levels {
                let level = book.level(id, seed, Mode::Classic, options)?;
                remap_without_old_board(progress, &level);
            }
            let level = book.level(
                state.hardcore.current_level,
                &format!("{seed}:hardcore"),
                Mode::Hardcore,
                LevelOptions::default(),
            )?;
            remap_without_old_board(&mut state.hardcore.progress, &level);
        } else if before_history_revision == state.dataset_revision
            && (state.hardcore.rules_revision != 2
                || state.hardcore.dataset_revision != state.dataset_revision)
        {
            let level = book.level(
                state.hardcore.current_level,
                &format!("{}:hardcore", state.campaign_seed),
                Mode::Hardcore,
                LevelOptions::default(),
            )?;
            state.hardcore.progress.revealed.clear();
            remap_without_old_board(&mut state.hardcore.progress, &level);
        }
        state.version = 2;
        state.generator_version = 6;
        state.word_revision = 1;
        state.difficulty_revision = 2;
        state.layout_version = 2;
        state.dataset_revision.clone_from(&book.dataset_revision);
        state
            .hardcore
            .dataset_revision
            .clone_from(&book.dataset_revision);
        state.hardcore.rules_revision = 2;
        normalize_economy(&mut state);

        let id = active_id(&state);
        let active = load_level(&book, &history, &state, state.mode, id)?;
        let recent = state.levels.keys().copied().collect();
        let mut game = Self {
            random: campaign_entropy(now_millis) | 1,
            rack: active.level.letters.clone(),
            active,
            state,
            history,
            recent,
            book,
            local_day,
            now_millis,
            dirty: raw_save.is_empty(),
            effects: Effects::default(),
        };
        game.reconcile_progress();
        Ok(game)
    }

    pub fn level(&self) -> &Level {
        &self.active.level
    }
    pub fn cells(&self) -> &[Cell] {
        &self.active.cells
    }
    pub fn is_hardcore(&self) -> bool {
        self.state.mode == Mode::Hardcore
    }
    pub fn progress(&self) -> &Progress {
        if self.is_hardcore() {
            &self.state.hardcore.progress
        } else {
            &self.state.levels[&self.state.current_level]
        }
    }
    fn progress_mut(&mut self) -> &mut Progress {
        if self.is_hardcore() {
            &mut self.state.hardcore.progress
        } else {
            self.state
                .levels
                .entry(self.state.current_level)
                .or_default()
        }
    }

    pub fn meanings(&self, word: &str) -> Vec<String> {
        self.book
            .meanings(word)
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    pub fn definitions(&self, word: &str) -> Vec<Meaning> {
        self.book
            .entry(word)
            .into_iter()
            .flat_map(|entry| &entry.definitions.ru)
            .filter(|sense| !sense.text.trim().is_empty())
            .map(|sense| Meaning {
                text: sense.text.clone(),
                part_of_speech: sense.part_of_speech.clone(),
            })
            .collect()
    }

    pub fn sources(&self, word: &str) -> Vec<String> {
        let mut paths = Vec::new();
        if let Some(entry) = self.book.entry(word) {
            for source in &entry.sources {
                for link in [&source.article_url, &source.page_url, &source.url]
                    .into_iter()
                    .flatten()
                {
                    if (link.starts_with("https://") || link.starts_with("http://"))
                        && !paths.contains(link)
                    {
                        paths.push(link.clone());
                    }
                }
            }
        }
        paths
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    #[cfg(any(target_os = "android", test))]
    pub(crate) fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    pub fn take_save(&mut self) -> Option<String> {
        if !self.dirty {
            return None;
        }
        let data = serde_json::to_string(&self.state).ok()?;
        if data.len() > save::SAVE_BYTES {
            return None;
        }
        self.dirty = false;
        Some(data)
    }

    pub fn take_effects(&mut self) -> Effects {
        std::mem::take(&mut self.effects)
    }

    fn signal(&mut self, sound: &'static str, haptic: bool) {
        if self.state.settings.sound {
            self.effects.sound = Some(sound);
        }
        self.effects.haptic |= haptic;
    }

    fn random_index(&mut self, count: usize) -> usize {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 7;
        self.random ^= self.random << 17;
        (self.random % count as u64) as usize
    }

    fn sampled_index(&mut self, count: usize, sample: Option<f64>) -> usize {
        sample.map_or_else(
            || self.random_index(count),
            |value| ((value * count as f64).floor().max(0.0) as usize).min(count - 1),
        )
    }

    fn reconcile_progress(&mut self) {
        let id = self.level().id;
        let hardcore = self.is_hardcore();
        let saved = if hardcore {
            Some(self.state.hardcore.progress.clone())
        } else {
            self.state.levels.get(&id).cloned()
        };
        let rewarded = if hardcore {
            self.state.hardcore.completed_count > 0 && saved.as_ref().is_some_and(|p| p.rewarded)
        } else {
            save::contains(&self.state.completed_ranges, id)
        };
        let mut progress = saved.clone().unwrap_or_default();
        let mut keys = HashSet::new();
        progress.revealed.retain(|key| {
            self.active.cells.iter().any(|cell| cell.key == *key) && keys.insert(key.clone())
        });
        if saved.is_none() && rewarded {
            progress.revealed = self
                .active
                .cells
                .iter()
                .map(|cell| cell.key.clone())
                .collect();
        }
        let old_solved = std::mem::take(&mut progress.solved);
        progress.solved = solved_words(&self.active.level, &progress.revealed);
        let allowed_bonus = |word: &str| {
            self.active
                .bonuses
                .iter()
                .position(|slot| slot.as_deref() == Some(word))
                .is_some_and(|index| {
                    hardcore
                        || self
                            .state
                            .bonus_ranges
                            .get(index)
                            .is_some_and(|r| save::contains(r, id))
                })
        };
        let mut seen = HashSet::new();
        progress
            .bonus
            .retain(|word| allowed_bonus(word) && seen.insert(word.clone()));
        if saved.is_none() && !hardcore {
            progress.bonus = self
                .active
                .bonuses
                .iter()
                .flatten()
                .filter(|word| allowed_bonus(word))
                .cloned()
                .collect();
        }
        progress.found_order = ordered_found(&progress, &old_solved);
        progress.completed = progress.solved.len() == self.active.level.words.len();
        progress.rewarded = rewarded;
        *self.progress_mut() = progress;
        if !hardcore {
            self.recent.retain(|&entry| entry != id);
            self.recent.push_back(id);
            while self.recent.len() > RECENT_LEVELS {
                if let Some(old) = self.recent.pop_front() {
                    self.state.levels.remove(&old);
                }
            }
        }
    }

    fn change_level(&mut self, mode: Mode, id: u64, reset: bool) -> bool {
        let Ok(active) = load_level(&self.book, &self.history, &self.state, mode, id) else {
            return false;
        };
        self.state.mode = mode;
        if mode == Mode::Hardcore {
            self.state.hardcore.current_level = id;
            if reset {
                self.state.hardcore.progress = Progress::default();
                self.state.hardcore.rewarded_words.clear();
            }
        } else {
            self.state.current_level = id;
            if reset {
                self.state.levels.insert(id, Progress::default());
            }
        }
        self.rack.clone_from(&active.level.letters);
        self.active = active;
        self.reconcile_progress();
        self.mark_dirty();
        true
    }

    pub fn set_mode(&mut self, mode: Mode) -> bool {
        let id = if mode == Mode::Hardcore {
            self.state.hardcore.current_level
        } else {
            self.state.current_level
        };
        if self.state.mode == mode {
            self.rack.clone_from(&self.active.level.letters);
            self.mark_dirty();
            true
        } else {
            self.change_level(mode, id, false)
        }
    }

    pub fn start_level(&mut self, id: u64) -> bool {
        id > 0 && id <= self.state.max_unlocked && self.change_level(Mode::Classic, id, false)
    }

    pub fn replay(&mut self, id: u64) -> bool {
        id > 0 && id <= self.state.max_unlocked && self.change_level(Mode::Classic, id, true)
    }

    pub fn regenerate_hardcore(&mut self) -> bool {
        let previous = self.active.level.id;
        if !self.is_hardcore() || previous == MAX_LEVEL {
            return false;
        }
        let mut old_letters = self.active.level.letters.clone();
        old_letters.sort_unstable();
        let seed = format!("{}:hardcore", self.state.campaign_seed);
        for candidate in previous + 1..=previous.saturating_add(20).min(MAX_LEVEL) {
            let Ok(level) =
                self.book
                    .level(candidate, &seed, Mode::Hardcore, LevelOptions::default())
            else {
                return false;
            };
            let mut letters = level.letters;
            letters.sort_unstable();
            if letters != old_letters || candidate == previous.saturating_add(20).min(MAX_LEVEL) {
                return self.change_level(Mode::Hardcore, candidate, true);
            }
        }
        false
    }

    pub fn next_level(&mut self) -> bool {
        if self.is_hardcore() {
            return self.progress().completed && self.regenerate_hardcore();
        }
        self.start_level(self.state.current_level.saturating_add(1))
    }

    pub fn shuffle(&mut self) {
        self.shuffle_with_samples(&[]);
    }

    pub fn shuffle_with_samples(&mut self, samples: &[f64]) {
        let original = self.rack.clone();
        for (index, i) in (1..self.rack.len()).rev().enumerate() {
            let j = self.sampled_index(i + 1, samples.get(index).copied());
            self.rack.swap(i, j);
        }
        if self.rack == original {
            if let Some(index) = self
                .rack
                .iter()
                .position(|letter| Some(letter) != self.rack.first())
            {
                self.rack.swap(0, index);
            }
        }
    }

    pub fn submit(&mut self, submitted: &str) -> ActionResult {
        let word = normalize_word(submitted);
        let length = word.chars().count();
        let buildable = can_build_word(&word, &self.active.level.letters);
        let progress = self.progress();
        let mut result = ActionResult::new("invalid");
        result.word.clone_from(&word);
        result.completed = progress.completed;
        if self.level().excluded_word_lengths.contains(&length) && buildable {
            result.kind = "excluded-length";
        } else if !self.book.has_word(&word) {
            result.kind = "invalid";
        } else if !buildable {
            result.kind = "unbuildable";
        } else if progress.solved.contains(&word) || progress.bonus.contains(&word) {
            result.kind = "repeat";
        } else if let Some(placement) = self
            .active
            .level
            .words
            .iter()
            .find(|placement| placement.word == word)
        {
            let keys: Vec<_> = placement
                .cells()
                .map(|(row, col, _)| format!("{row},{col}"))
                .collect();
            let progress = self.progress_mut();
            for key in keys {
                if !progress.revealed.contains(&key) {
                    progress.revealed.push(key);
                }
            }
            result.kind = "target";
            result.reward = self.sync(Some(&word));
            result.completed = self.progress().completed;
        } else if let Some(index) = self
            .active
            .bonuses
            .iter()
            .position(|slot| slot.as_ref() == Some(&word))
        {
            let progress = self.progress_mut();
            progress.bonus.push(word.clone());
            progress.found_order.push(word);
            result.kind = "bonus";
            if self.is_hardcore() {
                self.state.hardcore.bonus_count = capped_add(self.state.hardcore.bonus_count, 1);
            } else {
                let id = self.level().id;
                self.state
                    .bonus_ranges
                    .resize_with(self.state.bonus_ranges.len().max(index + 1), Vec::new);
                let ranges = &mut self.state.bonus_ranges[index];
                if !save::contains(ranges, id) {
                    save::insert(ranges, id);
                    result.reward = BONUS_REWARD;
                    self.state.coins = capped_add(self.state.coins, BONUS_REWARD);
                }
            }
        } else {
            result.kind = "outside";
        }
        match result.kind {
            "target" | "bonus" => {
                self.mark_dirty();
                let sound = if result.kind == "target" && result.completed {
                    "victory"
                } else {
                    result.kind
                };
                self.signal(sound, true);
            }
            "repeat" => self.signal("duplicate", false),
            _ => self.signal("negative", false),
        }
        result
    }

    fn sync(&mut self, entered: Option<&str>) -> u64 {
        let solved = solved_words(&self.active.level, &self.progress().revealed);
        let id = self.level().id;
        let complete = solved.len() == self.active.level.words.len();
        let words = self.active.level.words.len() as u64;
        let progress = self.progress_mut();
        let new: Vec<_> = solved
            .iter()
            .filter(|word| !progress.solved.contains(word))
            .cloned()
            .collect();
        let mut order = ordered_found(progress, &[]);
        order.extend(
            new.iter()
                .filter(|word| Some(word.as_str()) != entered)
                .cloned(),
        );
        if let Some(word) = entered.filter(|word| new.iter().any(|candidate| candidate == word)) {
            order.push(word.to_owned());
        }
        progress.solved = solved;
        progress.found_order = order;
        progress.completed = complete;
        let mut reward = 0;
        if self.is_hardcore() {
            let hardcore = &mut self.state.hardcore;
            for word in new {
                if !hardcore.rewarded_words.contains(&word) {
                    hardcore.rewarded_words.push(word);
                    hardcore.word_count = capped_add(hardcore.word_count, 1);
                }
            }
            if complete && !hardcore.progress.rewarded {
                hardcore.completed_count = capped_add(hardcore.completed_count, 1);
                reward = words * 6 + hardcore.progress.bonus.len() as u64 * 12;
                hardcore.reward_coins = capped_add(hardcore.reward_coins, reward);
                hardcore.progress.rewarded = true;
            }
        } else {
            if complete && !save::contains(&self.state.completed_ranges, id) {
                save::insert(&mut self.state.completed_ranges, id);
                reward = CLASSIC_REWARD;
            }
            let rewarded = save::contains(&self.state.completed_ranges, id);
            self.progress_mut().rewarded = rewarded;
            self.state.max_unlocked = unlocked_level(&self.state.completed_ranges);
        }
        self.state.coins = capped_add(self.state.coins, reward);
        reward
    }

    pub fn hint(&mut self) -> ActionResult {
        self.hint_with_sample(None)
    }

    pub fn hint_with_sample(&mut self, sample: Option<f64>) -> ActionResult {
        if self.is_hardcore() {
            return ActionResult::new("unavailable");
        }
        let revealed = &self.progress().revealed;
        let hidden: Vec<_> = self
            .active
            .cells
            .iter()
            .filter(|cell| !revealed.contains(&cell.key))
            .map(|cell| cell.key.clone())
            .collect();
        if hidden.is_empty() {
            return ActionResult::new("complete");
        }
        if self.state.coins < HINT_COST {
            return ActionResult::new("insufficient");
        }
        let index = self.sampled_index(hidden.len(), sample);
        self.progress_mut().revealed.push(hidden[index].clone());
        self.state.coins -= HINT_COST;
        self.state.hint_count = capped_add(self.state.hint_count, 1);
        let reward = self.sync(None);
        self.mark_dirty();
        self.signal(if self.progress().completed { "victory" } else { "hint" }, true);
        ActionResult {
            kind: "hint",
            reward,
            completed: self.progress().completed,
            word: String::new(),
        }
    }

    pub fn claim_gift(&mut self, day: i64) -> bool {
        self.effects.request_notifications = true;
        if !(0..=100_000_000).contains(&day)
            || self
                .state
                .daily_gift
                .last_claim_day
                .is_some_and(|last| last >= day)
        {
            return false;
        }
        self.state.daily_gift.count += 1;
        self.state.daily_gift.last_claim_day = Some(day);
        self.state.coins = capped_add(self.state.coins, GIFT_REWARD);
        self.mark_dirty();
        self.signal("bonus", true);
        true
    }

    pub fn purchase_background(&mut self, id: &str) -> ActionResult {
        let Some(background) = BACKGROUNDS.iter().find(|bg| bg.id == id) else {
            return ActionResult::new("invalid");
        };
        if self.state.backgrounds.owned.iter().any(|owned| owned == id) {
            return ActionResult::new("owned");
        }
        if self.state.coins < background.price {
            return ActionResult::new("insufficient");
        }
        self.state.coins -= background.price;
        self.state.backgrounds.owned.push(id.into());
        self.state.backgrounds.paid.push(id.into());
        self.select_background(id);
        self.signal("bonus", true);
        ActionResult::new("purchased")
    }

    pub fn select_background(&mut self, id: &str) -> bool {
        if !self.state.backgrounds.owned.iter().any(|owned| owned == id) {
            return false;
        }
        self.state.settings.background = Some(id.into());
        self.mark_dirty();
        true
    }

    pub fn reset(&mut self) {
        let mut settings = self.state.settings.clone();
        settings.background = settings.background.filter(|id| id == "ufa");
        let mut state = Save::new(
            self.state.campaign_seed.clone(),
            &self.book.dataset_revision,
        );
        state.settings = settings;
        state.legacy_layout_through_level = self.state.legacy_layout_through_level;
        state.bonus_layouts.clone_from(&self.state.bonus_layouts);
        if let Ok(active) = load_level(&self.book, &self.history, &state, Mode::Classic, 1) {
            self.state = state;
            self.rack.clone_from(&active.level.letters);
            self.active = active;
            self.recent.clear();
            self.reconcile_progress();
            self.mark_dirty();
        }
    }
}

pub(crate) fn level_options(state: &Save) -> LevelOptions {
    LevelOptions {
        layout_legacy_through: state.legacy_layout_through_level,
        ..LevelOptions::default()
    }
}

fn load_level(
    book: &PuzzleBook,
    history: &crate::history::History,
    state: &Save,
    mode: Mode,
    id: u64,
) -> Result<ActiveLevel, String> {
    if mode == Mode::Hardcore {
        let seed = format!("{}:hardcore", state.campaign_seed);
        let level = book.level(id, &seed, mode, LevelOptions::default())?;
        return Ok(ActiveLevel::new(level, book, None));
    }
    let level = book.level(id, &state.campaign_seed, mode, level_options(state))?;
    let legacy_bonus = history.bonuses(state, id)?;
    Ok(ActiveLevel::new(level, book, legacy_bonus))
}

fn active_id(state: &Save) -> u64 {
    if state.mode == Mode::Hardcore {
        state.hardcore.current_level
    } else {
        state.current_level
    }
}

fn unlocked_level(ranges: &[[u64; 2]]) -> u64 {
    ranges
        .first()
        .filter(|range| range[0] == 1)
        .map_or(1, |range| range[1].saturating_add(1).min(MAX_LEVEL))
}

fn ordered_found(progress: &Progress, earlier_solved: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    progress
        .found_order
        .iter()
        .chain(earlier_solved)
        .chain(&progress.solved)
        .chain(&progress.bonus)
        .filter(|word| {
            (progress.solved.contains(word) || progress.bonus.contains(word))
                && seen.insert((*word).clone())
        })
        .cloned()
        .collect()
}

fn solved_words(level: &Level, shown: &[String]) -> Vec<String> {
    let shown: HashSet<_> = shown.iter().map(String::as_str).collect();
    level
        .words
        .iter()
        .filter(|word| {
            word.cells()
                .all(|(row, col, _)| shown.contains(format!("{row},{col}").as_str()))
        })
        .map(|word| word.word.clone())
        .collect()
}

pub(crate) fn remap_without_old_board(progress: &mut Progress, level: &Level) {
    let desired = progress.revealed.len().min(level.cells().len());
    let mut seen = HashSet::new();
    let mut keys = Vec::new();
    for placement in level.words.iter().filter(|placement| {
        progress.solved.contains(&placement.word) || progress.completed && progress.rewarded
    }) {
        for (row, col, _) in placement.cells() {
            let key = format!("{row},{col}");
            if seen.insert(key.clone()) {
                keys.push(key);
            }
        }
    }
    for cell in level.cells() {
        if keys.len() >= desired {
            break;
        }
        let key = format!("{},{}", cell.row, cell.col);
        if seen.insert(key.clone()) {
            keys.push(key);
        }
    }
    progress.revealed = keys;
}

fn normalize_backgrounds(state: &mut Save) {
    if state.backgrounds.version != 1 {
        state.backgrounds = Backgrounds::default();
        state.backgrounds.owned = BACKGROUNDS
            .iter()
            .filter(|bg| bg.legacy_level <= state.max_unlocked)
            .map(|bg| bg.id.to_owned())
            .collect();
    }
    state.backgrounds.owned = BACKGROUNDS
        .iter()
        .filter(|bg| bg.price == 0 || state.backgrounds.owned.iter().any(|id| id == bg.id))
        .map(|bg| bg.id.to_owned())
        .collect();
    let owned = &state.backgrounds.owned;
    state.backgrounds.paid = BACKGROUNDS
        .iter()
        .filter(|bg| {
            bg.price > 0
                && owned.iter().any(|id| id == bg.id)
                && state.backgrounds.paid.iter().any(|id| id == bg.id)
        })
        .map(|bg| bg.id.to_owned())
        .collect();
    if state
        .settings
        .background
        .as_ref()
        .is_some_and(|id| !owned.contains(id))
    {
        state.settings.background = None;
    }
}

fn normalize_hardcore(hardcore: &mut Hardcore) {
    hardcore.completed_count = hardcore.completed_count.min(hardcore.current_level);
    hardcore.bonus_count = hardcore.bonus_count.min(hardcore.current_level * 3);
    hardcore.legacy_completed_count = hardcore
        .legacy_completed_count
        .min(hardcore.completed_count);
    hardcore.legacy_bonus_count = hardcore.legacy_bonus_count.min(hardcore.bonus_count);
    if hardcore.reward_revision < 2 {
        if hardcore.reward_revision == 0 {
            hardcore.legacy_completed_count = hardcore.completed_count;
            hardcore.legacy_bonus_count = hardcore.bonus_count;
            hardcore.word_count = 0;
            hardcore
                .rewarded_words
                .clone_from(&hardcore.progress.solved);
        }
        hardcore.legacy_earnings = hardcore
            .legacy_completed_count
            .saturating_mul(30)
            .saturating_add(
                (hardcore.completed_count - hardcore.legacy_completed_count).saturating_mul(500),
            )
            .saturating_add(hardcore.legacy_bonus_count.saturating_mul(5))
            .saturating_add(hardcore.bonus_count - hardcore.legacy_bonus_count)
            .saturating_add(hardcore.word_count)
            .min(MAX_NUMBER);
        hardcore.reward_coins = 0;
        hardcore.reward_revision = 2;
    }
    let mut seen = HashSet::new();
    hardcore
        .rewarded_words
        .retain(|word| normalize_word(word) == *word && seen.insert(word.clone()));
    hardcore.rewarded_words.truncate(12_976);
    hardcore.reward_coins = hardcore
        .reward_coins
        .min(hardcore.current_level.saturating_mul(12_976 * 18))
        .min(MAX_NUMBER);
}

fn normalize_economy(state: &mut Save) {
    let completed = save::count(&state.completed_ranges);
    let legacy_v1 = state
        .extra
        .get("legacy")
        .and_then(|legacy| legacy.get("version"))
        .and_then(serde_json::Value::as_u64)
        == Some(1)
        && state
            .extra
            .get("migration")
            .and_then(|migration| migration.get("fromVersion"))
            .and_then(serde_json::Value::as_u64)
            == Some(1)
        && state.economy.completed_baseline <= completed
        && state.economy.base_coins <= MAX_NUMBER;
    if !legacy_v1 {
        state.economy.base_coins = 90;
        state.economy.completed_baseline = 0;
    }
    let counted = completed - state.economy.completed_baseline;
    let older = if state.economy.reward_revision == 2 {
        state.economy.legacy_classic_completions.min(counted)
    } else {
        counted
    };
    state.economy.legacy_classic_completions = older;
    state.economy.reward_revision = 2;
    let bonuses = state
        .bonus_ranges
        .iter()
        .map(|ranges| save::count(ranges))
        .fold(0, u64::saturating_add);
    let expected = state
        .economy
        .base_coins
        .saturating_add((counted - older).saturating_mul(CLASSIC_REWARD))
        .saturating_add(older.saturating_mul(30))
        .saturating_add(bonuses.saturating_mul(BONUS_REWARD))
        .saturating_add(state.daily_gift.count.saturating_mul(GIFT_REWARD))
        .saturating_add(state.hardcore.legacy_earnings)
        .saturating_add(state.hardcore.reward_coins)
        .min(MAX_NUMBER);
    let background_spend: u64 = BACKGROUNDS
        .iter()
        .filter(|bg| state.backgrounds.paid.iter().any(|id| id == bg.id))
        .map(|bg| bg.price)
        .sum();
    state.coins = state.coins.min(
        expected
            .saturating_sub(state.hint_count.saturating_mul(HINT_COST))
            .saturating_sub(background_spend),
    );
}

fn capped_add(left: u64, right: u64) -> u64 {
    left.saturating_add(right).min(MAX_NUMBER)
}

fn campaign_entropy(now: i64) -> u64 {
    use std::collections::hash_map::RandomState;
    use std::hash::BuildHasher;
    RandomState::new().hash_one(now)
}
