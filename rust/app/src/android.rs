use crate::{puzzles::PuzzleBook, web_game::WebGame};
use jni::JNIEnv;
use jni::objects::{JByteArray, JObject, JString, JValue};
use jni::sys::jstring;
use serde_json::{Value, json};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, OnceLock};

static BOOK: OnceLock<Arc<PuzzleBook>> = OnceLock::new();
static GAME: Mutex<Option<WebGame>> = Mutex::new(None);

fn read_asset(env: &mut JNIEnv, assets: &JObject, path: &str) -> Result<String, String> {
    let java_path = env.new_string(path).map_err(|error| error.to_string())?;
    let bytes = env
        .call_static_method(
            "ru/bashkir/words/NativeBridge",
            "readAsset",
            "(Landroid/content/res/AssetManager;Ljava/lang/String;)[B",
            &[JValue::Object(assets), JValue::Object(java_path.as_ref())],
        )
        .and_then(|value| value.l())
        .map_err(|error| format!("{path}: {error}"))?;
    if bytes.is_null() {
        return Err(format!("Missing packaged resource: {path}"));
    }
    let data = env
        .convert_byte_array(JByteArray::from(bytes))
        .map_err(|error| error.to_string())?;
    String::from_utf8(data).map_err(|error| format!("{path}: {error}"))
}

fn load_book(env: &mut JNIEnv, assets: &JObject) -> Result<Arc<PuzzleBook>, String> {
    if let Some(book) = BOOK.get() {
        return Ok(Arc::clone(book));
    }
    let lexicon = read_asset(env, assets, "data/game-lexicon.nouns.json")?;
    let dictionary = read_asset(env, assets, "data/nouns.full.json")?;
    let pools = read_asset(env, assets, "data/level-pools.json")?;
    let book = Arc::new(PuzzleBook::from_json(&lexicon, &dictionary, &pools)?);
    let _ = BOOK.set(Arc::clone(&book));
    Ok(book)
}

fn dispatch(env: &mut JNIEnv, input: JString, assets: &JObject) -> Result<Value, String> {
    let text: String = env
        .get_string(&input)
        .map_err(|error| error.to_string())?
        .into();
    if text.len() > crate::save::SAVE_BYTES + 8_192 {
        return Err("Game request is too large".into());
    }
    let request: Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let mut game = GAME.lock().unwrap_or_else(|error| error.into_inner());
    if request.get("op").and_then(Value::as_str) == Some("init") {
        let book = load_book(env, assets)?;
        let mut next = WebGame::new(book, &request, |path| read_asset(env, assets, path))?;
        let response = next.connected();
        *game = Some(next);
        Ok(response)
    } else {
        game.as_mut()
            .ok_or_else(|| "Game session has not started".into())
            .and_then(|game| game.dispatch(&request))
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ru_bashkir_words_NativeBridge_nativeDispatch(
    mut env: JNIEnv,
    _class: JObject,
    input: JString,
    assets: JObject,
) -> jstring {
    let response = match catch_unwind(AssertUnwindSafe(|| dispatch(&mut env, input, &assets))) {
        Ok(Ok(response)) => response,
        Ok(Err(error)) => json!({"ok":false, "error":error}),
        Err(_) => json!({"ok":false, "error":"The native game could not process the request"}),
    };
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
    }
    env.new_string(response.to_string())
        .map_or(std::ptr::null_mut(), JString::into_raw)
}
