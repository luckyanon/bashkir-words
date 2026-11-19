use crate::game::{ActionResult, Game, Mode};
use crate::puzzles::PuzzleBook;
use crate::save::Save;
use serde_json::{Value, json};
use std::sync::Arc;

pub(crate) struct WebGame {
    game: Game,
}

impl WebGame {
    pub fn new(
        book: Arc<PuzzleBook>,
        request: &Value,
        load_archive: impl FnMut(&str) -> Result<String, String>,
    ) -> Result<Self, String> {
        let seed = request
            .get("seed")
            .and_then(Value::as_str)
            .filter(|seed| !seed.is_empty() && seed.len() <= 256)
            .ok_or("Missing campaign seed")?;
        let now = request
            .get("nowMillis")
            .and_then(Value::as_i64)
            .ok_or("Missing clock")?;
        let day = request
            .get("localDay")
            .and_then(Value::as_i64)
            .ok_or("Missing local day")?;

        let mut saved = request.get("saved").cloned().unwrap_or_default();
        if let Some(saved) = saved.as_object_mut() {
            saved.insert("campaignSeed".into(), json!(seed));
        } else {
            saved = json!(Save::new(seed.to_owned(), &book.dataset_revision));
        }
        let json = serde_json::to_string(&saved).map_err(|error| error.to_string())?;
        let game = Game::new_with_history(book, &json, now, day, load_archive)?;
        Ok(Self { game })
    }

    pub fn connected(&mut self) -> Value {
        self.response(Value::Null)
    }

    pub fn dispatch(&mut self, request: &Value) -> Result<Value, String> {
        if let Some(now) = request.get("nowMillis").and_then(Value::as_i64) {
            self.game.now_millis = now;
        }
        if let Some(day) = request.get("localDay").and_then(Value::as_i64) {
            self.game.local_day = day;
        }
        let op = request
            .get("op")
            .and_then(Value::as_str)
            .ok_or("Missing game operation")?;
        let result = match op {
            "submit" => action(
                self.game
                    .submit(request.get("word").and_then(Value::as_str).unwrap_or("")),
            ),
            "hint" => action(
                self.game
                    .hint_with_sample(request.get("sample").and_then(Value::as_f64)),
            ),
            "shuffle" => {
                let samples: Vec<_> = request
                    .get("samples")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .take(12)
                    .map(|value| value.as_f64().unwrap_or(0.0))
                    .collect();
                self.game.shuffle_with_samples(&samples);
                json!(&self.game.rack)
            }
            "setMode" => {
                let mode = match request.get("mode").and_then(Value::as_str) {
                    Some("classic") => Mode::Classic,
                    Some("hardcore") => Mode::Hardcore,
                    _ => return Err("Unsupported game mode".into()),
                };
                json!(self.game.set_mode(mode))
            }
            "startLevel" => json!(self.game.start_level(level_id(request))),
            "replay" => json!(self.game.replay(level_id(request))),
            "regenerateHardcore" => json!(self.game.regenerate_hardcore()),
            "claimDailyGift" => json!(self.game.claim_gift(self.game.local_day)),
            "purchaseBackground" => action(
                self.game
                    .purchase_background(request.get("id").and_then(Value::as_str).unwrap_or("")),
            ),
            "setSettings" => {
                if let Some(update) = request.get("update").and_then(Value::as_object) {
                    for (key, setting) in [
                        ("sound", &mut self.game.state.settings.sound),
                        ("music", &mut self.game.state.settings.music),
                        ("animations", &mut self.game.state.settings.animations),
                        (
                            "backgroundMotion",
                            &mut self.game.state.settings.background_motion,
                        ),
                    ] {
                        if let Some(value) = update.get(key).and_then(Value::as_bool) {
                            *setting = value;
                        }
                    }
                    if let Some(id) = update.get("background").and_then(Value::as_str) {
                        self.game.select_background(id);
                    }
                }
                self.game.mark_dirty();
                json!(self.game.state.settings)
            }
            "reset" => {
                self.game.reset();
                Value::Null
            }
            _ => return Err("Unsupported game operation".into()),
        };
        Ok(self.response(result))
    }

    fn response(&mut self, result: Value) -> Value {
        let changed = self.game.take_changed();
        json!({
            "ok": true,
            "state": self.game.state,
            "level": self.game.level(),
            "rack": self.game.rack,
            "result": result,
            "changed": changed,
        })
    }
}

fn level_id(request: &Value) -> u64 {
    request
        .get("id")
        .and_then(Value::as_u64)
        .unwrap_or_default()
}

fn action(action: ActionResult) -> Value {
    json!({"kind":action.kind, "word":action.word, "reward":action.reward, "completed":action.completed})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_play_uses_the_original_campaign_and_sampling() {
        let book = Arc::new(
            PuzzleBook::from_json(
                include_str!("../../../public/data/game-lexicon.nouns.json"),
                include_str!("../../../public/data/nouns.full.json"),
                include_str!("../../../public/data/level-pools.json"),
            )
            .unwrap(),
        );
        let mut session = WebGame::new(
            book,
            &json!({
                "seed":"restore-web-ui:Башҡортса",
                "nowMillis":1_795_078_800_000_i64, "localDay":20_776
            }),
            |_| Err("Unexpected archive".into()),
        )
        .unwrap();
        let initial = session.connected();
        assert_eq!(initial["state"]["coins"], 90);
        assert_eq!(initial["state"]["campaignSeed"], "restore-web-ui:Башҡортса");
        assert_eq!(initial["rack"], initial["level"]["letters"]);
        let hint = session.dispatch(&json!({"op":"hint", "sample":0})).unwrap();
        let first = session.game.cells().first().unwrap();
        assert_eq!(hint["state"]["levels"]["1"]["revealed"][0], first.key);
        assert_eq!(hint["state"]["coins"], 70);
        assert_eq!(hint["changed"], true);
        let hardcore = session
            .dispatch(&json!({"op":"setMode","mode":"hardcore"}))
            .unwrap();
        let rack = hardcore["rack"].clone();
        let shuffled = session
            .dispatch(&json!({"op":"shuffle", "samples":[0,0,0,0,0,0,0,0]}))
            .unwrap();
        assert_ne!(shuffled["rack"], rack);
        assert_eq!(shuffled["changed"], false);
        let returned = session
            .dispatch(&json!({"op":"setMode","mode":"hardcore"}))
            .unwrap();
        assert_eq!(returned["rack"], rack);
        assert_eq!(returned["changed"], true);
    }
}
