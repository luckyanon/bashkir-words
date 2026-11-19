use crate::{game::Game, puzzles::PuzzleBook, session::Session};
use jni::JNIEnv;
use jni::objects::{JByteArray, JClass, JObject, JString, JValue};
use jni::sys::{jint, jlong, jstring};
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicI64, Ordering},
};

static BOOK: OnceLock<Arc<PuzzleBook>> = OnceLock::new();
static SESSIONS: OnceLock<Mutex<HashMap<i64, Session>>> = OnceLock::new();
static NEXT_SESSION: AtomicI64 = AtomicI64::new(1);

fn read_asset(env: &mut JNIEnv, assets: &JObject, path: &str) -> Result<String, String> {
    let path_java = env.new_string(path).map_err(|error| error.to_string())?;
    let bytes = env
        .call_static_method(
            "ru/bashkir/words/NativeBridge",
            "readAsset",
            "(Landroid/content/res/AssetManager;Ljava/lang/String;)[B",
            &[JValue::Object(assets), JValue::Object(path_java.as_ref())],
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

fn create_session(
    env: &mut JNIEnv,
    raw_save: JString,
    now: jlong,
    day: jint,
    assets: &JObject,
) -> Result<jlong, String> {
    let save = if raw_save.is_null() {
        String::new()
    } else {
        env.get_string(&raw_save)
            .map_err(|error| error.to_string())?
            .into()
    };
    let book = if let Some(book) = BOOK.get() {
        Arc::clone(book)
    } else {
        let lexicon = read_asset(env, assets, "data/game-lexicon.nouns.json")?;
        let dictionary = read_asset(env, assets, "data/nouns.full.json")?;
        let pools = read_asset(env, assets, "data/level-pools.json")?;
        let book = Arc::new(PuzzleBook::from_json(&lexicon, &dictionary, &pools)?);
        let _ = BOOK.set(Arc::clone(&book));
        book
    };
    let game = Game::new_with_history(book, &save, now, i64::from(day), |path| {
        read_asset(env, assets, path)
    })?;
    let handle = NEXT_SESSION.fetch_add(1, Ordering::Relaxed);
    let mut sessions = SESSIONS
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    sessions.insert(handle, Session::new(game));
    Ok(handle)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ru_bashkir_words_NativeBridge_create(
    mut env: JNIEnv,
    _class: JClass,
    save: JString,
    now: jlong,
    day: jint,
    assets: JObject,
) -> jlong {
    match catch_unwind(AssertUnwindSafe(|| {
        create_session(&mut env, save, now, day, &assets)
    })) {
        Ok(Ok(handle)) => handle,
        Ok(Err(error)) => {
            if !env.exception_check().unwrap_or(true) {
                let _ = env.throw_new("java/lang/IllegalStateException", error);
            }
            0
        }
        Err(_) => {
            let _ = env.throw_new(
                "java/lang/IllegalStateException",
                "Не удалось загрузить игру",
            );
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ru_bashkir_words_NativeBridge_dispatch(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    input: JString,
) -> jstring {
    let response = catch_unwind(AssertUnwindSafe(|| {
        let input: String = env.get_string(&input).ok()?.into();
        let event = serde_json::from_str(&input).ok()?;
        let mut sessions = SESSIONS
            .get()?
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        serde_json::to_string(&sessions.get_mut(&handle)?.dispatch(event)).ok()
    }))
    .ok()
    .flatten()
    .unwrap_or_else(|| "{}".into());
    env.new_string(response)
        .map_or(std::ptr::null_mut(), JString::into_raw)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ru_bashkir_words_NativeBridge_destroy(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if let Some(sessions) = SESSIONS.get() {
        sessions
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&handle);
    }
}
