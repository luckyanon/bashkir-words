use bashkir_words::{
    game::Game,
    puzzles::{Level, Mode, PuzzleBook},
    save::Save,
};
use serde::Deserialize;
use serde_json::Value;
use std::sync::{Arc, OnceLock};

fn book() -> Arc<PuzzleBook> {
    static BOOK: OnceLock<Arc<PuzzleBook>> = OnceLock::new();
    BOOK.get_or_init(|| {
        Arc::new(
            PuzzleBook::from_json(
                include_str!("../../../public/data/game-lexicon.nouns.json"),
                include_str!("../../../public/data/nouns.full.json"),
                include_str!("../../../public/data/level-pools.json"),
            )
            .unwrap(),
        )
    })
    .clone()
}

#[derive(Deserialize)]
struct Fixture {
    from: String,
    saved: Value,
    expected: Save,
    classic: Level,
    hardcore: Level,
}

#[test]
fn imports_capacitor_progress_and_older_archive_then_reloads() {
    let fixtures: Vec<Fixture> = serde_json::from_str(include_str!("fixtures/saves.json")).unwrap();
    for fixture in fixtures {
        let original = serde_json::to_string(&fixture.saved).unwrap();
        let mut game = Game::new(book(), &original, 1_795_078_800_000, 20_776).unwrap();
        assert_eq!(game.state.coins, fixture.expected.coins, "{}", fixture.from);
        assert_eq!(game.level(), &fixture.hardcore, "{}", fixture.from);
        assert_eq!(
            game.state.hardcore, fixture.expected.hardcore,
            "{}",
            fixture.from
        );
        assert!(game.set_mode(Mode::Classic));
        assert_eq!(game.level(), &fixture.classic, "{}", fixture.from);
        assert!(game.set_mode(Mode::Hardcore));
        assert_eq!(game.state, fixture.expected, "{}", fixture.from);
        let current = game.take_save().unwrap();
        let reloaded = Game::new(book(), &current, 1_795_078_800_000, 20_776).unwrap();
        assert_eq!(reloaded.state, game.state);
    }
}

#[test]
fn classic_and_hardcore_share_money_and_pay_only_once() {
    let mut game = Game::new(book(), "", 1_795_078_800_000, 20_776).unwrap();
    assert_eq!(game.state.coins, 90);
    assert!(game.claim_gift(game.local_day));
    assert!(!game.claim_gift(game.local_day));
    assert_eq!(game.hint().kind, "hint");
    assert_eq!(game.state.hint_count, 1);
    let targets: Vec<_> = game
        .level()
        .words
        .iter()
        .map(|item| item.word.clone())
        .collect();
    for word in &targets {
        game.submit(word);
    }
    assert!(game.progress().completed);
    let before_replay = game.state.coins;
    assert!(game.replay(1));
    for word in targets {
        game.submit(&word);
    }
    assert_eq!(game.state.coins, before_replay);

    assert!(game.set_mode(Mode::Hardcore));
    assert_eq!(game.hint().kind, "unavailable");
    let targets: Vec<_> = game
        .level()
        .words
        .iter()
        .map(|item| item.word.clone())
        .collect();
    for word in &targets {
        game.submit(word);
    }
    assert!(game.progress().completed);
    assert_eq!(game.state.coins, before_replay + targets.len() as u64 * 6);
    assert!(game.regenerate_hardcore());
    assert!(game.progress().found_order.is_empty());
    assert!(game.set_mode(Mode::Classic));
    assert!(game.progress().completed);
}
