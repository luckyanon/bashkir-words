use crate::game::{align_bonus, bonus_words, level_options};
use crate::puzzles::{Level, LevelOptions, Mode, PuzzleBook};
use crate::save::{BonusLayout, MAX_NUMBER, Progress, Save};
use std::collections::{HashMap, HashSet};

const FIRST_ARCHIVE: &str = "nouns-2026-10-19-rules-2";
fn archive_path(revision: &str) -> Option<&'static str> {
    match revision {
        FIRST_ARCHIVE => Some("data/migrations/2.9.0.json"),
        "nouns-2026-11-01-review-1" => Some("data/migrations/2.9.4.json"),
        "3e289d7b8e8e501e" => Some("data/migrations/2.9.5.json"),
        "0f4edc6e8c823fa9" => Some("data/migrations/2.10.0.json"),
        "6987d31ccbc93291" => Some("data/migrations/2.10.2.json"),
        "dd0523ec354e3366" => Some("data/migrations/2.10.3.json"),
        _ => None,
    }
}

#[derive(Default)]
pub struct History {
    old_books: HashMap<String, PuzzleBook>,
}

impl History {
    pub fn restore(
        save: &mut Save,
        current: &PuzzleBook,
        mut load: impl FnMut(&str) -> Result<String, String>,
    ) -> Result<Self, String> {
        let mut history = Self::default();
        if save.generator_version != 6 {
            save.bonus_layouts.clear();
            save.extra.remove("bonusLayoutThroughLevel");
            return Ok(history);
        }

        if save.bonus_layouts.is_empty() {
            if let Some(through_level) = save
                .extra
                .remove("bonusLayoutThroughLevel")
                .and_then(|value| value.as_u64())
            {
                save.bonus_layouts.push(BonusLayout {
                    dataset_revision: FIRST_ARCHIVE.into(),
                    through_level,
                });
            }
        }
        save.bonus_layouts.retain(|layout| {
            (1..MAX_NUMBER).contains(&layout.through_level)
                && archive_path(&layout.dataset_revision).is_some()
        });
        save.bonus_layouts.truncate(8);
        let revision = save.dataset_revision.clone();
        let replacing_archive =
            revision != current.dataset_revision && archive_path(&revision).is_some();
        if replacing_archive {
            let through_level = save.current_level.max(save.max_unlocked);
            if let Some(last) = save
                .bonus_layouts
                .last_mut()
                .filter(|last| last.dataset_revision == revision)
            {
                last.through_level = last.through_level.max(through_level);
            } else {
                save.bonus_layouts.push(BonusLayout {
                    dataset_revision: revision.clone(),
                    through_level,
                });
            }
        }
        for layout in &save.bonus_layouts {
            if history.old_books.contains_key(&layout.dataset_revision) {
                continue;
            }
            let path = archive_path(&layout.dataset_revision).expect("validated archive revision");
            let book = PuzzleBook::from_archive(current, &load(path)?)?;
            if book.dataset_revision != layout.dataset_revision {
                return Err(format!("Unexpected archive revision in {path}"));
            }
            history
                .old_books
                .insert(layout.dataset_revision.clone(), book);
        }

        let options = level_options(save);
        let seed = &save.campaign_seed;
        if replacing_archive {
            let previous = &history.old_books[&revision];
            for (&id, progress) in &mut save.levels {
                let before = previous.level(id, seed, Mode::Classic, options)?;
                let after = current.level(id, seed, Mode::Classic, options)?;
                remap(progress, &before, &after, true);
            }
            let id = save.hardcore.current_level;
            let seed = format!("{seed}:hardcore");
            let before = previous.level(id, &seed, Mode::Hardcore, LevelOptions::default())?;
            let after = current.level(id, &seed, Mode::Hardcore, LevelOptions::default())?;
            remap(&mut save.hardcore.progress, &before, &after, true);
            save.hardcore
                .dataset_revision
                .clone_from(&current.dataset_revision);
            save.dataset_revision.clone_from(&current.dataset_revision);
        }

        if save.dataset_revision == current.dataset_revision
            && (save.word_revision != 1 || save.difficulty_revision != 2)
        {
            let old_options = LevelOptions {
                include_local_words: save.word_revision > 0,
                apply_level_difficulty: save.difficulty_revision > 0,
                ..options
            };
            for (&id, progress) in &mut save.levels {
                let before = current.level(id, seed, Mode::Classic, old_options)?;
                let after = current.level(id, seed, Mode::Classic, options)?;
                remap(progress, &before, &after, false);
            }
        }
        Ok(history)
    }

    pub fn bonuses(&self, save: &Save, id: u64) -> Result<Option<Vec<Option<String>>>, String> {
        let mut slots: Option<Vec<Option<String>>> = None;
        for layout in save
            .bonus_layouts
            .iter()
            .filter(|layout| id <= layout.through_level)
        {
            let Some(old) = self.old_books.get(&layout.dataset_revision) else {
                continue;
            };
            let level = old.level(id, &save.campaign_seed, Mode::Classic, level_options(save))?;
            let words = bonus_words(&level, old);
            slots = Some(if let Some(previous) = slots {
                align_bonus(&previous, &words)
            } else {
                words
            });
        }
        Ok(slots)
    }
}

fn remap(progress: &mut Progress, before: &Level, after: &Level, replace_paid_cells: bool) {
    if before.words == after.words {
        return;
    }
    let shown: HashSet<_> = progress.revealed.iter().map(String::as_str).collect();
    let positions: HashMap<_, Vec<bool>> = before
        .words
        .iter()
        .map(|word| {
            (
                word.word.as_str(),
                word.cells()
                    .map(|(row, col, _)| shown.contains(format!("{row},{col}").as_str()))
                    .collect(),
            )
        })
        .collect();
    let mut revealed = Vec::new();
    let mut seen = HashSet::new();
    for word in &after.words {
        if let Some(known) = positions.get(word.word.as_str()) {
            for ((row, col, _), visible) in word.cells().zip(known) {
                if *visible && seen.insert((row, col)) {
                    revealed.push(format!("{row},{col}"));
                }
            }
        }
    }
    if replace_paid_cells {
        let desired = if progress.completed && progress.rewarded {
            after.cells().len()
        } else {
            shown.len()
        };
        for (row, col, _) in after.words.iter().flat_map(|word| word.cells()) {
            if revealed.len() >= desired {
                break;
            }
            if seen.insert((row, col)) {
                revealed.push(format!("{row},{col}"));
            }
        }
    }
    if progress.found_order.is_empty() {
        progress.found_order = progress
            .solved
            .iter()
            .chain(&progress.bonus)
            .cloned()
            .collect();
    }
    progress.revealed = revealed;
    progress.solved.clear();
}
