//! The Chumby's half of the backend (backend-gpl/chumby.cpp): the screen, the
//! touchscreen and top button, the soft buttons, the speaker and the save quota.
//! ScummVM calls these from its main thread, but for the chum_audio_* functions,
//! which its mixer thread calls.

use std::collections::VecDeque;
use std::ffi::{CStr, CString, c_char};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

use neochumby::audio::Stream;
use neochumby::fb::{self, Fb, rgb};
use neochumby::input::{Squeeze, Touch, Touchscreen};

use crate::games::Game;
use crate::gesture::{Gesture, Out};
use crate::quota;

unsafe extern "C" {
    fn chum_scummvm_main(argc: i32, argv: *const *const c_char) -> i32;
}

/// The master volume while a game plays; the game's own is applied in the mix.
const VOLUME: u32 = 80;
/// The soft buttons' row: under the overlay and the bar below a 320x200 game.
const BAR_Y: i32 = 220;
const BAR_H: i32 = fb::H as i32 - BAR_Y;
const BAR: [(&str, Action); 5] = [
    ("MENU", Action::Menu),
    ("ESC", Action::Key(KEY_ESC, KEY_ESC)),
    ("F5", Action::Key(KEY_F5, 0)),
    ("KEYS", Action::Keyboard),
    ("RCLK", Action::ArmRight),
];
const KEY_ESC: i32 = 27;
const KEY_F5: i32 = 286;
const KEY_BACKSPACE: i32 = 8;
const KEY_RETURN: i32 = 13;
const KEY_SPACE: i32 = 32;

const KB_ROWS: [&str; 4] = ["1234567890", "qwertyuiop", "asdfghjkl<", "zxcvbnm_>^"];
const KB_ROW_H: i32 = 22;
const KB_Y: i32 = BAR_Y - 4 * KB_ROW_H;
const KB_CELL_W: i32 = fb::W as i32 / 10;
const KEY_MS: u64 = 40;

#[derive(Clone, Copy)]
enum Action {
    Menu,
    Key(i32, i32),
    Keyboard,
    ArmRight,
}

#[repr(C)]
pub struct Frame {
    screen: *const u8,
    pitch: i32,
    w: i32,
    h: i32,
    palette: *const u16,
    overlay: *const u16,
    overlay_on: i32,
    dirty_top: i32,
    dirty_bottom: i32,
    shake: i32,
    cursor: *const u8,
    cursor_w: i32,
    cursor_h: i32,
    cursor_x: i32,
    cursor_y: i32,
    cursor_key: u32,
    cursor_palette: *const u16,
    cursor_on: i32,
}

#[repr(C)]
pub struct Event {
    kind: i32,
    x: i32,
    y: i32,
    key: i32,
    ascii: i32,
}

const EV_MOVE: i32 = 1;
const EV_LDOWN: i32 = 2;
const EV_LUP: i32 = 3;
const EV_RDOWN: i32 = 4;
const EV_RUP: i32 = 5;
const EV_KEYDOWN: i32 = 6;
const EV_KEYUP: i32 = 7;
const EV_MENU: i32 = 8;
const EV_QUIT: i32 = 9;

/// A touch that came down on the soft buttons or keyboard, not the game.
#[derive(Clone, Copy, PartialEq)]
enum Finger {
    None,
    Game,
    Soft(i32, i32),
}

struct State {
    fb: Fb,
    touch: Touchscreen,
    button: Squeeze,
    button_was: bool,
    button_checked: u64,
    start: Instant,
    gesture: Gesture,
    finger: Finger,
    keyboard: bool,
    /// The bar or keyboard changed, or the whole screen is to be drawn again.
    redraw: bool,
    was_overlay: bool,
    was_h: i32,
    /// Keys and the menu, each with the time it is due.
    pending: VecDeque<(u64, i32, i32, i32)>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
static AUDIO: Mutex<Option<Stream>> = Mutex::new(None);
static TERMINATED: AtomicBool = AtomicBool::new(false);

fn state() -> MutexGuard<'static, Option<State>> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

extern "C" fn on_term(_: i32) {
    TERMINATED.store(true, Ordering::Relaxed);
}

/// Where the game's rows start: a 320x200 game has a bar over and under it.
fn top(frame_overlay: bool, h: i32) -> i32 {
    if frame_overlay { 0 } else { ((fb::H as i32 - h) / 2).max(0) }
}

/// Runs a game in this process, and doesn't return.
pub fn run_game(app: &Path, game: &Game) -> ! {
    // SIGTERM asks the game to quit, so that the stream's drop mutes the
    // speaker; the session kills it soon after if it doesn't.
    // SAFETY: the handler only stores to an atomic, which is async-signal-safe.
    unsafe { libc::signal(libc::SIGTERM, on_term as *const () as libc::sighandler_t) };

    let fb = match Fb::open() {
        Ok(f) => f,
        Err(e) => {
            eprintln!("neochumby-scummvm: /dev/fb0: {e}");
            std::process::exit(1);
        }
    };
    let mut button = Squeeze::open();
    let button_was = button.squeezed();
    *state() = Some(State {
        fb,
        touch: Touchscreen::open(),
        button,
        button_was,
        button_checked: 0,
        start: Instant::now(),
        gesture: Gesture::default(),
        finger: Finger::None,
        keyboard: false,
        redraw: true,
        was_overlay: false,
        was_h: 0,
        pending: VecDeque::new(),
    });

    // /tmp is the app's own, and goes with the boot: nothing but saves lasts.
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let ini = format!("{home}/scummvm.ini");
    if let Err(e) = std::fs::write(&ini, include_str!("scummvm.ini")) {
        eprintln!("neochumby-scummvm: {ini}: {e}");
    }
    let saves = neochumby::session::storage().map_or_else(|| home.clone().into(), |s| s);
    let args = [
        "scummvm".to_string(),
        format!("--config={ini}"),
        format!("--path={}", game.dir(app).display()),
        format!("--extrapath={}", app.join("engine-data").display()),
        format!("--savepath={}", saves.display()),
        game.id.to_string(),
    ];
    let args: Vec<CString> = args.iter().map(|a| CString::new(a.as_str()).unwrap()).collect();
    let mut argv: Vec<*const c_char> = args.iter().map(|a| a.as_ptr()).collect();
    argv.push(std::ptr::null());
    // SAFETY: argv is args.len() NUL-terminated strings and a null, which live
    // to the end of this function; ScummVM doesn't return from this but by
    // exiting (OSystem_Chumby::quit).
    let code = unsafe { chum_scummvm_main(args.len() as i32, argv.as_ptr()) };
    chum_shutdown();
    std::process::exit(code)
}

fn draw_bar(f: &mut Fb, armed: bool) {
    let w = fb::W as i32 / BAR.len() as i32;
    f.rect(0, BAR_Y as usize, fb::W, BAR_H as usize, rgb(20, 20, 24));
    for (i, (label, action)) in BAR.iter().enumerate() {
        let x = i as i32 * w;
        let lit = armed && matches!(action, Action::ArmRight);
        let back = if lit { rgb(180, 120, 30) } else { rgb(44, 48, 56) };
        f.rect(x as usize + 1, BAR_Y as usize + 1, w as usize - 2, BAR_H as usize - 2, back);
        let tx = x + (w - Fb::text_width(label, 1) as i32) / 2;
        f.text(tx as usize, BAR_Y as usize + 6, label, 1, rgb(230, 230, 230));
    }
}

fn draw_keyboard(f: &mut Fb) {
    f.rect(0, KB_Y as usize, fb::W, (BAR_Y - KB_Y) as usize, rgb(12, 12, 16));
    for (r, row) in KB_ROWS.iter().enumerate() {
        for (c, ch) in row.chars().enumerate() {
            let (x, y) = (c as i32 * KB_CELL_W, KB_Y + r as i32 * KB_ROW_H);
            f.rect(x as usize + 1, y as usize + 1, KB_CELL_W as usize - 2, KB_ROW_H as usize - 2, rgb(50, 54, 64));
            let label = match ch {
                '<' => "BS".to_string(),
                '>' => "OK".to_string(),
                '_' => "SP".to_string(),
                '^' => "UP".to_string(),
                c => c.to_ascii_uppercase().to_string(),
            };
            let tx = x + (KB_CELL_W - Fb::text_width(&label, 1) as i32) / 2;
            f.text(tx as usize, y as usize + 7, &label, 1, rgb(235, 235, 235));
        }
    }
}

/// Draws a frame. Only rows that changed: a whole screen is about 200 KB to
/// uncached memory on a slow bus.
///
/// # Safety
/// `frame` and what it points to are as chumby.cpp's updateScreen makes them:
/// valid for its sizes, and not written by another thread during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chum_present(frame: *const Frame) {
    // SAFETY: as the caller promises.
    let frame = unsafe { &*frame };
    let mut guard = state();
    let Some(s) = guard.as_mut() else { return };
    let overlay = frame.overlay_on != 0;
    let h = if overlay { BAR_Y } else { frame.h.min(fb::H as i32) };
    let w = frame.w.min(fb::W as i32) as usize;
    let y0 = top(overlay, h);
    let mode_changed = overlay != s.was_overlay || h != s.was_h;
    let full = s.redraw || mode_changed;
    let (mut from, mut to) = if full { (0, h) } else { (frame.dirty_top.max(0), frame.dirty_bottom.min(h)) };
    if frame.shake != 0 && !overlay {
        (from, to) = (0, h);
    }
    // The keyboard covers the screen's lower rows.
    let limit = if s.keyboard { KB_Y } else { BAR_Y.max(y0 + h).min(fb::H as i32) };

    let px = s.fb.pixels();
    if full {
        // The bars, and whatever was on the screen before.
        px.fill(0);
    }
    let screen_w = fb::W;
    if from < to {
        for row in from..to {
            let dy = y0 + row + if overlay { 0 } else { frame.shake };
            if dy < 0 || dy >= limit {
                continue;
            }
            let dst = &mut px[dy as usize * screen_w..dy as usize * screen_w + w];
            if overlay {
                // SAFETY: the overlay is 320 x BAR_Y u16s, and row < BAR_Y.
                let src = unsafe { std::slice::from_raw_parts(frame.overlay.add(row as usize * screen_w), w) };
                dst.copy_from_slice(src);
            } else {
                // SAFETY: the screen is frame.h rows of frame.pitch bytes, at
                // least w wide, and row < h <= frame.h. The palette is 256 u16s,
                // so a u8 indexes it.
                unsafe {
                    let src = std::slice::from_raw_parts(frame.screen.add(row as usize * frame.pitch as usize), w);
                    for (d, p) in dst.iter_mut().zip(src) {
                        *d = *frame.palette.add(*p as usize);
                    }
                }
            }
        }
    }
    if frame.cursor_on != 0 {
        let key = frame.cursor_key as u8;
        for cy in 0..frame.cursor_h {
            let dy = y0 + frame.cursor_y + cy;
            if dy < 0 || dy >= limit {
                continue;
            }
            for cx in 0..frame.cursor_w {
                let dx = frame.cursor_x + cx;
                if dx < 0 || dx >= fb::W as i32 {
                    continue;
                }
                // SAFETY: the cursor is cursor_w x cursor_h bytes, and its
                // palette 256 u16s; cx and cy are in range.
                unsafe {
                    let p = *frame.cursor.add((cy * frame.cursor_w + cx) as usize);
                    if p != key {
                        px[dy as usize * screen_w + dx as usize] = *frame.cursor_palette.add(p as usize);
                    }
                }
            }
        }
    }
    if full {
        draw_bar(&mut s.fb, s.gesture.arm_right);
        if s.keyboard {
            draw_keyboard(&mut s.fb);
        }
        s.redraw = false;
        s.was_overlay = overlay;
        s.was_h = h;
    }
}

fn now_ms(s: &State) -> u64 {
    s.start.elapsed().as_millis() as u64
}

fn soft_hit(s: &State, x: i32, y: i32) -> Option<Action> {
    if y >= BAR_Y {
        return BAR.get((x * BAR.len() as i32 / fb::W as i32).clamp(0, BAR.len() as i32 - 1) as usize).map(|b| b.1);
    }
    if s.keyboard && y >= KB_Y {
        let row = KB_ROWS[((y - KB_Y) / KB_ROW_H).clamp(0, 3) as usize];
        let c = row.as_bytes()[(x / KB_CELL_W).clamp(0, 9) as usize];
        return Some(match c {
            b'<' => Action::Key(KEY_BACKSPACE, KEY_BACKSPACE),
            b'>' => Action::Key(KEY_RETURN, KEY_RETURN),
            b'_' => Action::Key(KEY_SPACE, KEY_SPACE),
            b'^' => Action::Keyboard,
            c => Action::Key(c as i32, c as i32),
        });
    }
    None
}

fn act(s: &mut State, action: Action, now: u64) {
    match action {
        Action::Menu => s.pending.push_back((now, EV_MENU, 0, 0)),
        Action::Key(key, ascii) => {
            s.pending.push_back((now, EV_KEYDOWN, key, ascii));
            s.pending.push_back((now + KEY_MS, EV_KEYUP, key, ascii));
        }
        Action::Keyboard => {
            s.keyboard = !s.keyboard;
            s.redraw = true;
        }
        Action::ArmRight => {
            s.gesture.arm_right = !s.gesture.arm_right;
            s.redraw = true;
        }
    }
}

/// The next event for ScummVM, 1 if there is one, or 0.
///
/// # Safety
/// `event` points to an Event, writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chum_poll(event: *mut Event, overlay_on: i32, game_h: i32) -> i32 {
    let mut guard = state();
    let Some(s) = guard.as_mut() else { return 0 };
    let now = now_ms(s);
    let mut out = Event { kind: 0, x: 0, y: 0, key: 0, ascii: 0 };
    if TERMINATED.swap(false, Ordering::Relaxed) {
        out.kind = EV_QUIT;
    } else {
        let overlay = overlay_on != 0;
        let h = if overlay { BAR_Y } else { game_h };
        let y0 = top(overlay, h);
        for t in s.touch.poll() {
            match t {
                Touch::Down { x, y, .. } => {
                    if s.finger == Finger::None {
                        s.finger = if soft_hit(s, x, y).is_some() { Finger::Soft(x, y) } else { Finger::Game };
                    }
                    if s.finger == Finger::Game {
                        s.gesture.touch(now, x, (y - y0).clamp(0, h - 1), overlay);
                    }
                }
                Touch::Up => {
                    match s.finger {
                        Finger::Game => s.gesture.lift(now),
                        Finger::Soft(x, y) => {
                            if let Some(a) = soft_hit(s, x, y) {
                                act(s, a, now);
                            }
                        }
                        Finger::None => {}
                    }
                    s.finger = Finger::None;
                }
            }
        }
        // A read of the switch is a system call: not on every poll.
        if now >= s.button_checked + 50 {
            s.button_checked = now;
            let pressed = s.button.squeezed();
            if pressed && !s.button_was {
                if overlay {
                    act(s, Action::Keyboard, now);
                } else {
                    act(s, Action::Menu, now);
                }
            }
            s.button_was = pressed;
        }
        if s.pending.front().is_some_and(|p| p.0 <= now) {
            let (_, kind, key, ascii) = s.pending.pop_front().unwrap();
            out = Event { kind, x: 0, y: 0, key, ascii };
        } else if let Some(e) = s.gesture.next(now) {
            let (kind, x, y) = match e {
                Out::Move(x, y) => (EV_MOVE, x, y),
                Out::LeftDown(x, y) => (EV_LDOWN, x, y),
                Out::LeftUp(x, y) => (EV_LUP, x, y),
                Out::RightDown(x, y) => (EV_RDOWN, x, y),
                Out::RightUp(x, y) => (EV_RUP, x, y),
            };
            out = Event { kind, x, y, key: 0, ascii: 0 };
        }
    }
    let got = out.kind != 0;
    // SAFETY: as the caller promises.
    unsafe { *event = out };
    i32::from(got)
}

/// ScummVM moved the pointer itself: nothing to do, as the next touch says
/// where the finger is. Here so that a touch screen's backend says so.
#[unsafe(no_mangle)]
pub extern "C" fn chum_warp(_x: i32, _y: i32) {}

#[unsafe(no_mangle)]
pub extern "C" fn chum_audio_open(rate: i32, channels: i32) {
    *AUDIO.lock().unwrap_or_else(|e| e.into_inner()) = Some(Stream::new(channels as u32, rate as u32, VOLUME));
}

/// # Safety
/// `pcm` points to `samples` i16s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chum_audio_write(pcm: *const i16, samples: i32) {
    // SAFETY: as the caller promises: the mixer thread's own buffer.
    let pcm = unsafe { std::slice::from_raw_parts(pcm, samples as usize) };
    if let Some(s) = AUDIO.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        s.write(pcm);
    }
}

/// Frames mixed that the speaker hasn't played yet.
#[unsafe(no_mangle)]
pub extern "C" fn chum_audio_buffered() -> i32 {
    AUDIO
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_mut()
        .map_or(i32::MAX, |s| (s.written() - s.played().min(s.written())) as i32)
}

/// Before the process ends: the stream's drop mutes the speaker and gives it up.
#[unsafe(no_mangle)]
pub extern "C" fn chum_shutdown() {
    *AUDIO.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// `data` as a gzip file, in a buffer of `*out` bytes for chum_gzip_free.
///
/// # Safety
/// `data` points to `len` bytes, and `out` to a writable int.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chum_gzip(data: *const u8, len: i32, out: *mut i32) -> *mut u8 {
    // SAFETY: as the caller promises.
    let gz = quota::gzip(unsafe { std::slice::from_raw_parts(data, len as usize) }).into_boxed_slice();
    // SAFETY: as the caller promises.
    unsafe { *out = gz.len() as i32 };
    Box::into_raw(gz).cast()
}

/// # Safety
/// `data` and `len` are what chum_gzip returned and set, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chum_gzip_free(data: *mut u8, len: i32) {
    // SAFETY: as the caller promises: a boxed slice of that length.
    drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(data, len as usize)) });
}

/// Whether a save of `size` bytes named `name` fits the app's storage at `dir`.
///
/// # Safety
/// `dir` and `name` are NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chum_storage_fits(dir: *const c_char, name: *const c_char, size: i32) -> i32 {
    // SAFETY: as the caller promises, for the length of the call.
    let (dir, name) = unsafe { (CStr::from_ptr(dir), CStr::from_ptr(name)) };
    let (Ok(dir), Ok(name)) = (dir.to_str(), name.to_str()) else { return 0 };
    i32::from(quota::fits(Path::new(dir), name, size as u64))
}
