# neochumby-scummvm

[ScummVM](https://www.scummvm.org) as a [neochumby](https://github.com/johnmerchant/neochumby)
app for the Chumby Classic (Linux 7 only): point-and-click adventure games on a
320x240 touchscreen, with the app's own small game list, the speaker, and saves in
the app's `storage`.

An independent project, not affiliated with or endorsed by the ScummVM team, Chumby
Industries, Blue Octy, anyone holding rights in Chumby, or any game's rights
holders. "Chumby" is a trademark of its owner. **It has been run only on the
emulator so far, never on a real Chumby.**

## Adding it to a tree

From a [neochumby](https://github.com/johnmerchant/neochumby) checkout, in `nix
develop` (zig and Rust, as neochumby's README says):

```sh
# the games, into a directory of the shape tools/tree's -d takes
git clone --recursive https://github.com/johnmerchant/neochumby-scummvm
neochumby-scummvm/tools/fetch-games /tmp/data        # or name some: sky queen ...

tools/tree /tmp/tree -d /tmp/data \
  'git+https://github.com/johnmerchant/neochumby-scummvm#FULL_COMMIT_ID'
```

`tools/tree` checks out the ScummVM submodule itself, which takes about 3 GB in its
cache: `shallow` in `.gitmodules` doesn't help, as git fetches a release tag's commit
with its whole history when that isn't a branch's tip. A first build takes a minute or
so on a fast machine. Without `-d` the app is there,
and lists every game as not installed. The whole set is 106 MB on the stick.

## What's in it

| Game | Engine | Stick | |
| --- | --- | --- | --- |
| Beneath a Steel Sky (floppy) | sky | 8.5 MB | |
| Flight of the Amazon Queen (floppy) | queen | 22 MB | |
| Lure of the Temptress | lure | 7 MB | |
| Drascula: The Vampire Strikes Back | drascula | 32 MB | No music: that's a separate add-on, needing a decoder this build leaves out |
| Dreamweb (floppy, UK) | dreamweb | 22 MB | Its terminals want typing: the keys |
| Soltys | cge | 8 MB | |
| Nippon Safes, Inc. (DOS) | parallaction | 6.5 MB | |

The game data isn't in this repository: `tools/fetch-games` fetches each from
ScummVM's official download, checked by hash. [docs/games.md](docs/games.md) has
each game's licence, source and hash, and why it's here.

The games' licences are their own, in a file beside each game. Copyright in each stays
with Revolution Software Ltd (Steel Sky, Lure), John Passfield and
Steven Stamatiadis (Amazon Queen), Alcachofa Soft S.L. (Drascula), Neil Dodwell and
David Dew trading as Creative Reality (Dreamweb), Laboratorium Komputerowe Avalon
(Soltys) and Dynabyte Software (Nippon Safes).

## Using it

- **The list** opens a game with a tap. The top button leaves it.
- **In a game:** a tap moves the pointer and clicks. A touch held still for 0.6
  seconds is the right button, until you lift it. A slide only moves the pointer, to
  read the names of things.
- **The bar along the bottom:** MENU (ScummVM's menu: save, load, options, quit),
  ESC, F5 (some games' own menu: Steel Sky's and Queen's), KEYS (a keyboard), RCLK
  (the next tap is a right click).
- **The top button** opens the menu or, inside ScummVM's menu, the keyboard.
- **Holding the top-left corner for 3 seconds** is neochumby's way out, as for any
  app: anything not saved is lost.
- "Return to Launcher" and Quit in the menu come back to the list.

What the left and right buttons do is each game's own: its manual or readme says.
Lure's right-button menu, for one, is up for as long as the finger stays down. Slide
to a choice, then lift.

## Saves

Saves go to the directory neochumby gives an app with `storage` (`NEOCHUMBY_STORAGE`).
Nothing else lasts: settings are in `/tmp`, and are the same at each start.

At the neochumby commit this was built against that directory is `/psp/apps/scummvm`,
and `/psp` on Linux 7 is a copy in RAM of what the stock boot handed over (Linux 7 has
no NAND driver): **this app never writes the Chumby's flash, but until neochumby's
stick layout lands, saves don't outlast a power cycle.** In that layout the same
variable names a RAM directory which the session copies to the stick
(`/mnt/usb/neochumby/state/apps/scummvm`) every 30 seconds and as it ends. This app
needs no change for that. That layout was read from work in progress, so check it
when it lands.

The limit either way is **64 KiB for the whole app, all games together**, and the
session ends an app that goes over. So this app:

- compresses saves with gzip, which ScummVM reads by itself;
- checks a finished save against the limit first: one that wouldn't fit fails, as a
  failed save in that game does, and nothing is written. Delete another in the Load
  or Save dialog to make room;
- never autosaves.

See "Measured", below, for what a save takes.

## Licences

- The code written for this repository (`src/`, `build.rs`, `tools/`, the docs) is
  [MIT](LICENSE).
- ScummVM (`third_party/scummvm`, a submodule, unchanged, at release v2026.4.0) is
  GPL-3.0-or-later, with its own `COPYING` and `LICENSES/`.
- `backend-gpl/` is written after a ScummVM backend, and stays GPL-3.0-or-later with
  its notice ([backend-gpl/COPYING](backend-gpl/COPYING)).
- **A built binary links ScummVM and is distributed under GPL-3.0-or-later as a
  whole.** The MIT code is compatible with that, but the combination is not MIT.
  `Cargo.toml` says `MIT AND GPL-3.0-or-later` for that reason. The app's directory
  carries the GPL and a notice saying where its source is (`files/`). If you pass a
  binary on, you pass on those obligations: the source, at the commit it was built
  from.
- The games are their rights holders', under the licence beside each, and aren't
  covered by any of the above.

This is the project's reading, not legal advice.

ScummVM has [guidelines on AI use](third_party/scummvm/AI-GUIDELINES.md) for
contributions to it. This repository contains AI-assisted code, and nothing here has
been offered upstream. Read them before offering any of it.

## How it's built

ScummVM's `configure` runs with the `null` backend for its build system, seven
engines and none of its optional libraries: no OpenGL, no network, no codecs, no
16-bit colour modes, no scalers. Its objects are linked with `backend-gpl/chumby.cpp`
in place of the null backend, and that calls the Rust side (`src/backend.rs`) for the
framebuffer, touch, top button, speaker and quota, through neochumby's crate. ScummVM
is never changed.

ScummVM's desktop launcher isn't shown. `src/launcher.rs` is the list, and each game
is a child process started with its game id, so a game exits to the list with all
of its memory given back.

`tools/tree`'s C compiler is `tools/zigcc`; `tools/zigcxx` is its C++ equivalent. The
Rust tests need no device: `cargo test` on a host runs them, and skips ScummVM.

## Measured

On the emulator, at ScummVM v2026.4.0:

| | |
| --- | --- |
| Binary, stripped, static | 4.0 MB (text 3.97 MB, data 51 KB, bss 1.1 MB) |
| The list, while a game runs | 0.7 MB resident |
| Least `MemAvailable` seen, with Drascula | 32 MB |

| Game | Peak resident, early in play | One save, compressed (raw) |
| --- | --- | --- |
| Beneath a Steel Sky | 7.3 MB | 12.7 KB + 46 bytes of index (56.7 KB) |
| Flight of the Amazon Queen | 6.2 MB | not measured |
| Lure of the Temptress | 7.5 MB | 11.4 KB |
| Drascula | 14.0 MB | not measured |
| Dreamweb | 8.7 MB | not measured: its menu refuses, and it saves on its own screens |
| Soltys | 5.8 MB | not measured |
| Nippon Safes | 5.4 MB | not measured |

So about 5 saves of Steel Sky or Lure fit, across all games together. Peaks are from
the first minutes of play, not whole games, and leave out the page cache and the
kernel's own use. Well under the 37 MB a running app has, on what was measured.

## Tested

All on `dev/emulate --profile dist`, against neochumby's session, in its jail (uid
20001, a mount namespace of its own). `cargo test` runs the gesture and quota logic
on a host.

| Game | Starts and draws | Responds to a tap |
| --- | --- | --- |
| Beneath a Steel Sky | yes | Foster walks to it |
| Flight of the Amazon Queen | yes | Joe walks to it; a verb is chosen in the panel |
| Lure of the Temptress | yes | Walks to it. A hold opens "Look through" on a window |
| Drascula | yes, with a dialog about CD music to tap OK | Advances the talk; in the first room, a hold opens the inventory |
| Dreamweb | yes | Tapping the bed shows its description |
| Soltys | yes | The farmer answers; the pointer follows |
| Nippon Safes | yes | Chooses a language, then a new or restored game |

Also: the list, and opening a game from it. The top button opening the menu in a
game, and leaving from the list. "Return to Launcher" coming back to the list. The
corner hold ending a game, and the speaker muting with it. A save and a restore in
Steel Sky, with gzip. Typing a save's name with the keyboard. A save that wouldn't
fit, refused and logged, with the app left running: with more than the limit put there
by hand, the session ended the app, which is why that check exists. Sound: a capture
from the emulator's speaker through Steel Sky's intro has a peak of 13074 and is 88%
non-zero.

**Not run:** a real Chumby, or any real speed. Any game played to its end. A save in
Queen, Drascula, Dreamweb, Soltys or Nippon Safes. How each engine shows a refused
save (it's logged either way). The top button opening the keyboard inside the menu.
Saves lasting across a power cycle (see "Saves").

## Known limits

- **Speed is a guess.** QEMU says nothing about a 266 MHz ARM926. All seven are early
  to mid 1990s DOS games, written for a 386 or 486, and no engine here needs an FPU.
  What is likelier to be slow is around the engines: AdLib music is emulated in
  software on the same CPU, and a stick under 1 MB/s is read in the same thread that
  draws.
  From most to least worried, by what each does and not by measurement: Drascula (the
  largest memory use, full-screen animation read from one 32 MB file), Dreamweb,
  Amazon Queen and Steel Sky (AdLib music all through), then Lure, Soltys and Nippon
  Safes. If music is too much, it can be turned off in ScummVM's options, for that
  session: settings don't last.
- **Saves are few, and don't last yet** (see "Saves"). 64 KiB is neochumby's limit for
  any app, and the owner's to change.
- **The screen is small for ScummVM's menu.** Its menus work at 320x220, with a built-in
  theme, but its dialogs are made for a mouse. The keyboard covers the lower part of the
  screen, including the field being typed in if it's there.
- **Drascula asks about CD music at each start,** since settings don't last: tap OK.
  Its music add-on isn't supported.
- **ESC is each game's own.** In Drascula it can leave the game. In Lure it asks to
  quit, with y or n on the keyboard.
- **Dreamweb saves on its own screens,** and needs the keyboard for its terminals. Its
  licence allows no modification, so don't compress its files.
- **Text and resolution:** nothing is scaled. English versions only.
- **The stick:** a game's first start reads files for detection, which on a real stick
  will take a moment.
