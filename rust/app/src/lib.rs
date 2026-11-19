pub mod game;
mod history;
pub mod puzzles;
pub mod save;
#[cfg(any(target_os = "android", test))]
mod web_game;

#[cfg(target_os = "android")]
mod android;
