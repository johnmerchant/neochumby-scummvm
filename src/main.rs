//! ScummVM on the Chumby Classic. Run by the session, this is the app's own
//! game list; each game runs as a child process (`--game ID`), so that it
//! starts from a clean heap and the list costs next to nothing while it plays.

// FFI to ScummVM (C++, GPL), as neochumby's Doom and Quake are allowed.
// Each unsafe block says why it holds.
#![allow(unsafe_code)]
// Off the Chumby, only the tests use these modules.
#![cfg_attr(not(target_arch = "arm"), allow(dead_code))]

#[cfg(target_arch = "arm")]
mod backend;
mod games;
mod gesture;
#[cfg(target_arch = "arm")]
mod launcher;
mod quota;

#[cfg(target_arch = "arm")]
fn main() {
    neochumby::debug::install("neochumby-scummvm");
    // Jailed there's no /proc to find the binary by, but the session runs it
    // in its own directory.
    let dir = std::env::current_dir().unwrap_or_else(|_| ".".into());
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("--game") => {
            let Some(game) = args.get(2).and_then(|id| games::find(id)) else {
                eprintln!("neochumby-scummvm: no such game");
                std::process::exit(2);
            };
            backend::run_game(&dir, game);
        }
        _ => launcher::run(&dir),
    }
}

#[cfg(not(target_arch = "arm"))]
fn main() {
    eprintln!("neochumby-scummvm runs on the Chumby; on this machine there are only its tests");
}
