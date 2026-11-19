use crate::game::Game;
use crate::ui::Ui;
use serde_json::{Value, json};

pub(crate) struct Session {
    game: Game,
    ui: Ui,
    foreground: bool,
}

impl Session {
    pub fn new(game: Game) -> Self {
        Self {
            game,
            ui: Ui::default(),
            foreground: true,
        }
    }

    pub fn dispatch(&mut self, event: Value) -> Value {
        if let Some(now) = event.get("nowMillis").and_then(Value::as_i64) {
            self.game.now_millis = now;
        }
        if let Some(day) = event.get("localDay").and_then(Value::as_i64) {
            self.game.local_day = day;
        }
        match event.get("type").and_then(Value::as_str) {
            Some("resume") => self.foreground = true,
            Some("pause") => self.foreground = false,
            _ => {}
        }

        self.ui.event(&mut self.game, event);
        let mut response = self.ui.frame(&self.game);
        let Some(frame) = response.as_object_mut() else {
            return json!({"commands":[]});
        };
        if let Value::Object(effects) = self.ui.take_effects() {
            frame.extend(effects);
        }
        let effects = self.game.take_effects();
        if let Some(sound) = effects.sound {
            frame.insert("sound".into(), json!(sound));
        }
        if !self.game.state.settings.sound {
            frame.remove("sound");
        }
        if effects.haptic {
            frame.insert("haptic".into(), json!("success"));
        }
        if effects.request_notifications {
            frame.insert("requestNotifications".into(), json!(true));
        }
        if let Some(save) = self.game.take_save() {
            frame.insert("save".into(), json!(save));
        }
        frame.insert(
            "claimedDay".into(),
            json!(self.game.state.daily_gift.last_claim_day.unwrap_or(-1)),
        );
        frame.insert(
            "motion".into(),
            json!(self.foreground && self.game.state.settings.background_motion),
        );
        frame.insert(
            "music".into(),
            json!(self.foreground && self.game.state.settings.music),
        );
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::puzzles::PuzzleBook;
    use std::sync::Arc;

    #[test]
    fn android_frames_carry_only_new_saves_and_respect_lifecycle() {
        let book = PuzzleBook::from_json(
            include_str!("../../../public/data/game-lexicon.nouns.json"),
            include_str!("../../../public/data/nouns.full.json"),
            include_str!("../../../public/data/level-pools.json"),
        )
        .unwrap();
        let game = Game::new(Arc::new(book), "", 1_795_078_800_000, 20_776).unwrap();
        let mut session = Session::new(game);

        let frame = session.dispatch(json!({"type":"resize","width":390,"height":844}));
        assert!(!frame["commands"].as_array().unwrap().is_empty());
        assert!(frame["save"].as_str().is_some());
        let gift = frame["controls"]
            .as_array()
            .unwrap()
            .iter()
            .find(|control| control["label"] == "Подарок")
            .unwrap()["id"]
            .clone();
        let claim = session.dispatch(json!({"type":"activate","id":gift,"localDay":20_776}));
        let state: Value = serde_json::from_str(claim["save"].as_str().unwrap()).unwrap();
        assert_eq!(state["coins"], 190);
        assert_eq!(claim["claimedDay"], 20_776);
        assert_eq!(claim["requestNotifications"], true);

        let paused = session.dispatch(json!({"type":"pause"}));
        assert!(paused.get("save").is_none());
        assert_eq!(paused["music"], false);
        assert_eq!(paused["motion"], false);
    }
}
