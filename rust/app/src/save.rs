use crate::puzzles::Mode;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const MAX_NUMBER: u64 = 9_007_199_254_740_991;
pub const SAVE_BYTES: usize = 2_000_000;
pub type Ranges = Vec<[u64; 2]>;
pub type Extra = BTreeMap<String, Value>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub sound: bool,
    pub music: bool,
    pub animations: bool,
    pub background_motion: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sound: true,
            music: true,
            animations: true,
            background_motion: true,
            background: None,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct DailyGift {
    pub count: u64,
    pub last_claim_day: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_claim_at: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Backgrounds {
    pub version: u32,
    pub owned: Vec<String>,
    pub paid: Vec<String>,
}

impl Default for Backgrounds {
    fn default() -> Self {
        Self {
            version: 1,
            owned: vec!["ufa".into()],
            paid: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Progress {
    pub revealed: Vec<String>,
    pub solved: Vec<String>,
    pub bonus: Vec<String>,
    pub found_order: Vec<String>,
    pub completed: bool,
    pub rewarded: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Hardcore {
    pub current_level: u64,
    pub completed_count: u64,
    pub bonus_count: u64,
    pub word_count: u64,
    pub reward_revision: u32,
    pub reward_coins: u64,
    pub legacy_earnings: u64,
    pub legacy_completed_count: u64,
    pub legacy_bonus_count: u64,
    pub rewarded_words: Vec<String>,
    pub progress: Progress,
    pub dataset_revision: String,
    pub rules_revision: u32,
}

impl Default for Hardcore {
    fn default() -> Self {
        Self {
            current_level: 1,
            completed_count: 0,
            bonus_count: 0,
            word_count: 0,
            reward_revision: 2,
            reward_coins: 0,
            legacy_earnings: 0,
            legacy_completed_count: 0,
            legacy_bonus_count: 0,
            rewarded_words: Vec::new(),
            progress: Progress::default(),
            dataset_revision: String::new(),
            rules_revision: 2,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Economy {
    pub base_coins: u64,
    pub completed_baseline: u64,
    pub reward_revision: u32,
    pub legacy_classic_completions: u64,
}

impl Default for Economy {
    fn default() -> Self {
        Self {
            base_coins: 90,
            completed_baseline: 0,
            reward_revision: 2,
            legacy_classic_completions: 0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BonusLayout {
    pub dataset_revision: String,
    pub through_level: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Save {
    pub version: u32,
    pub campaign_seed: String,
    pub generator_version: u32,
    pub current_level: u64,
    pub max_unlocked: u64,
    pub coins: u64,
    pub settings: Settings,
    pub levels: BTreeMap<u64, Progress>,
    pub completed_ranges: Ranges,
    pub bonus_ranges: Vec<Ranges>,
    pub hint_count: u64,
    pub economy: Economy,
    pub daily_gift: DailyGift,
    pub backgrounds: Backgrounds,
    pub legacy_through_level: u64,
    pub legacy_v3_through_level: u64,
    pub layout_version: u32,
    pub legacy_layout_through_level: u64,
    pub dataset_revision: String,
    pub word_revision: u32,
    pub difficulty_revision: u32,
    pub mode: Mode,
    pub hardcore: Hardcore,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub bonus_layouts: Vec<BonusLayout>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for Save {
    fn default() -> Self {
        Self {
            version: 2,
            campaign_seed: String::new(),
            generator_version: 6,
            current_level: 1,
            max_unlocked: 1,
            coins: 90,
            settings: Settings::default(),
            levels: BTreeMap::new(),
            completed_ranges: Vec::new(),
            bonus_ranges: vec![Vec::new(); 3],
            hint_count: 0,
            economy: Economy::default(),
            daily_gift: DailyGift::default(),
            backgrounds: Backgrounds::default(),
            legacy_through_level: 0,
            legacy_v3_through_level: 0,
            layout_version: 2,
            legacy_layout_through_level: 0,
            dataset_revision: String::new(),
            word_revision: 1,
            difficulty_revision: 2,
            mode: Mode::Classic,
            hardcore: Hardcore::default(),
            bonus_layouts: Vec::new(),
            extra: Extra::new(),
        }
    }
}

impl Save {
    pub fn new(seed: String, dataset_revision: &str) -> Self {
        Self {
            campaign_seed: seed,
            dataset_revision: dataset_revision.into(),
            hardcore: Hardcore {
                dataset_revision: dataset_revision.into(),
                ..Hardcore::default()
            },
            ..Self::default()
        }
    }
}

pub fn decode(raw: &str) -> Option<Save> {
    if raw.len() > SAVE_BYTES {
        return None;
    }
    let value: Value = serde_json::from_str(raw).ok()?;
    if !matches!(value.get("version")?.as_u64()?, 1 | 2) || !value.get("levels")?.is_object() {
        return None;
    }
    let had_layout = value.get("layoutVersion").is_some();
    let had_word_revision = value.get("wordRevision").is_some();
    let had_difficulty = value.get("difficultyRevision").is_some();
    let had_backgrounds = value.get("backgrounds").is_some();
    let old_hardcore_reward = value
        .get("hardcore")
        .is_some_and(|hardcore| hardcore.get("rewardRevision").is_none());
    let old_economy = value
        .get("economy")
        .is_some_and(|economy| economy.get("rewardRevision").is_none());
    let mut save: Save = serde_json::from_value(value).ok()?;
    if !had_layout {
        save.layout_version = 1;
    }
    if !had_word_revision {
        save.word_revision = 0;
    }
    if !had_difficulty {
        save.difficulty_revision = 0;
    }
    if !had_backgrounds {
        save.backgrounds.version = 0;
    }
    if old_hardcore_reward {
        save.hardcore.reward_revision = 0;
    }
    if old_economy {
        save.economy.reward_revision = 0;
    }
    Some(save)
}

pub fn contains(ranges: &[[u64; 2]], id: u64) -> bool {
    let index = ranges.partition_point(|[start, _]| *start <= id);
    index > 0 && id <= ranges[index - 1][1]
}

pub fn count(ranges: &[[u64; 2]]) -> u64 {
    ranges
        .iter()
        .map(|[start, end]| end - start + 1)
        .fold(0, u64::saturating_add)
}

pub fn normalize(ranges: &mut Ranges, upper: u64) {
    ranges.truncate(100_000);
    ranges.retain(|[start, end]| *start > 0 && *start <= *end && *end <= upper);
    ranges.sort_unstable();
    let mut merged: Ranges = Vec::with_capacity(ranges.len());
    for &[start, end] in ranges.iter() {
        if let Some(last) = merged.last_mut().filter(|last| start <= last[1] + 1) {
            last[1] = last[1].max(end);
        } else {
            merged.push([start, end]);
        }
    }
    *ranges = merged;
}

pub fn insert(ranges: &mut Ranges, id: u64) {
    if !contains(ranges, id) {
        ranges.push([id, id]);
        normalize(ranges, MAX_NUMBER - 1);
    }
}
