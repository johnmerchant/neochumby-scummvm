//! The app's own game list: a row for each game, a tap opens it. The top button
//! leaves, as the corner hold does. Each game runs as a child; the list comes
//! back when it ends.

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use neochumby::fb::{self, Fb, rgb};
use neochumby::input::{BUTTON_ENV, Squeeze, Touch, Touchscreen};

use crate::games::{GAMES, Game};

const HEAD: usize = 18;
const ROW: usize = (fb::H - HEAD) / GAMES.len();

fn draw(f: &mut Fb, app: &Path) {
    f.clear(rgb(16, 22, 16));
    f.rect(0, 0, fb::W, HEAD - 2, rgb(63, 155, 60));
    // Starts clear of the top-left corner, which is the session's way out.
    f.text(48, 4, "SCUMMVM", 1, rgb(255, 255, 255));
    let mut any = false;
    for (i, g) in GAMES.iter().enumerate() {
        let y = HEAD + i * ROW;
        let here = installed(g, app);
        any |= here;
        let (title, detail) =
            if here { (rgb(240, 240, 220), rgb(150, 170, 150)) } else { (rgb(90, 90, 90), rgb(70, 70, 70)) };
        f.text(8, y + 2, g.title, 2, title);
        f.text(8, y + 19, if here { g.detail } else { "not installed: tools/fetch-games" }, 1, detail);
        f.rect(0, y + ROW - 1, fb::W, 1, rgb(40, 50, 40));
    }
    if !any {
        eprintln!("neochumby-scummvm: no games in games/: run tools/fetch-games and add them with tools/tree -d");
    }
}

fn installed(g: &Game, app: &Path) -> bool {
    g.dir(app).is_dir()
}

/// The game a tap at (x, y) opens.
fn row(y: i32) -> Option<&'static Game> {
    let y = usize::try_from(y).ok()?.checked_sub(HEAD)?;
    GAMES.get(y / ROW)
}

/// A descriptor of the top button for the games. neochumby keeps the session's
/// from the programs an app runs, and without /proc a jailed game couldn't find
/// the device for itself. So this copy, which isn't close-on-exec, stays open.
fn button_for_games() -> Option<i32> {
    let fd: i32 = std::env::var(BUTTON_ENV).ok()?.parse().ok()?;
    // SAFETY: F_DUPFD only makes a new descriptor, above those a process
    // usually has; a bad fd makes it fail, which is checked.
    let copy = unsafe { libc::fcntl(fd, libc::F_DUPFD, 64) };
    (copy >= 0).then_some(copy)
}

pub fn run(app: &Path) -> ! {
    let button = button_for_games();
    loop {
        match choose(app) {
            Some(g) => play(g.id, button),
            None => std::process::exit(0),
        }
    }
}

/// Shows the list until a game is chosen, or the top button leaves.
fn choose(app: &Path) -> Option<&'static Game> {
    let mut f = match Fb::open() {
        Ok(f) => f,
        Err(e) => {
            eprintln!("neochumby-scummvm: /dev/fb0: {e}");
            std::process::exit(1);
        }
    };
    draw(&mut f, app);
    let mut touch = Touchscreen::open();
    let mut button = Squeeze::open();
    let mut was = button.squeezed();
    let mut at = None;
    // A tap meant for the game that just ended isn't one for the list, whose
    // rows it may fall on.
    let opened = Instant::now();
    loop {
        for t in touch.poll() {
            match t {
                Touch::Down { y, .. } => at = Some(y),
                Touch::Up if opened.elapsed() < Duration::from_millis(800) => at = None,
                Touch::Up => {
                    if let Some(g) = at.take().and_then(row).filter(|g| installed(g, app)) {
                        return Some(g);
                    }
                }
            }
        }
        let now = button.squeezed();
        if now && !was {
            return None;
        }
        was = now;
        std::thread::sleep(Duration::from_millis(30));
    }
}

/// Runs a game to its end. The list's screen and devices were dropped first.
fn play(id: &str, button: Option<i32>) {
    let mut game = Command::new("./neochumby-scummvm");
    game.arg("--game").arg(id);
    if let Some(fd) = button {
        game.env(BUTTON_ENV, fd.to_string());
    }
    // A failure is logged, and the list is back: nothing here is fatal.
    match game.status() {
        Ok(s) if s.success() => {}
        Ok(s) => eprintln!("neochumby-scummvm: {id} ended: {s}"),
        Err(e) => eprintln!("neochumby-scummvm: {id}: {e}"),
    }
}
