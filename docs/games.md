# Which games, and why

**This repository contains and redistributes no game data.** `tools/fetch-games`
downloads each game from ScummVM's official download site, checks it against the
SHA-256 below, and unpacks it, licence included, for your own tree. This page
records what was read to decide which games the fetch script offers. It is a
reading of the licence files by the project, not legal advice, and it has not had
a lawyer's review.

## What each game's permission says

The primary source for each game is the licence file inside the archive ScummVM
distributes. All seven archives were downloaded and those files read. The
"Permission text" column quotes clause 1 of each.

ScummVM's [games page](https://www.scummvm.org/games/), section "Download freeware
games", is where the URLs and hashes come from. It presents these as freeware, and
carries no licence text of its own.

Base for the URLs is `https://downloads.scummvm.org/frs/extras/`.

| Game | Copyright, as the licence file states it | Licence file in the archive | Permission text | Source URL | SHA-256 of the archive | Decision |
| --- | --- | --- | --- | --- | --- | --- |
| Beneath a Steel Sky, floppy | Revolution Software Ltd | `readme.txt`, "The Legal Stuff" | "You may distribute this game for free on any medium, provided this readme and all associated copyright notices and disclaimers are left intact." | `Beneath a Steel Sky/BASS-Floppy-1.3.zip` | `d0bac1bd61747a67e885fa44b78c78887bf2b15d3dfa2790c483fad651078818` | **Included** |
| Flight of the Amazon Queen, floppy | John Passfield and Steven Stamatiadis | `readme.txt`, "The Legal Stuff" | "You may distribute this game for free on any medium, provided this Readme and all associated copyright notices and disclaimers are left intact." | `Flight of the Amazon Queen/FOTAQ_Floppy.zip` | `2e59de85f708cdb32bf85c85b394ac091c05f7647e856b71f5b3ae73fde761e0` | **Included** |
| Lure of the Temptress, English | Revolution Software Ltd | `LICENSE.txt` | "You may distribute this game for free on any medium, provided this license and all associated copyright notices and disclaimers are left intact." | `Lure of the Temptress/lure-1.1.zip` | `f3178245a1483da1168c3a11e70b65d33c389f1f5df63d4f3a356886c1890108` | **Included** |
| Drascula: The Vampire Strikes Back, English | Alcachofa Soft S.L. | `readme.txt`, "The Legal Stuff" | "You may distribute "DRASCULA" for free on any medium, provided this Readme and all associated copyright notices and disclaimers are left intact." | `Drascula_ The Vampire Strikes Back/drascula-1.0.zip` | `b731f6cb5a22ba8b4c3b3362f570b9a10a67b6cb0b395394b19a94b36e4e42de` | **Included** (see the note) |
| Dreamweb, floppy, English (UK) | Neil Dodwell and David Dew trading as Creative Reality | `license.txt` | "You may distribute this game for free on any medium, provided this license and all associated copyright notices and disclaimers are left intact." | `Dreamweb/dreamweb-uk-1.1.zip` | `0f98e5fbeedfff5f5b904220d3bd3288da5a272f9192da03f2160c5edb8cdfb3` | **Included** (see the note) |
| Soltys, English | Laboratorium Komputerowe Avalon | `license.txt` | "You may distribute this game for free on any medium, provided this license and all associated copyright notices and disclaimers are left intact." | `Soltys/soltys-en-v1.0.zip` | `87b89e654b8a5b8ebe342cb4c5c6049ab9a43a5efb474d9c49bafb77dcce48f6` | **Included** |
| Nippon Safes, Inc., DOS | Dynabyte Software | `readme.txt` | "You may distribute this game for free on any medium, provided this license and all associated copyright notices and disclaimers are left intact." | `Nippon Safes/nippon-1.0.zip` | `53e7e2c60065e4aed193169bbcdcfd1113fa68d3efe1c8240ba073c0e20d613f` | **Included** (see the note) |

Other versions on the same page, with the same kind of licence, are **left out for
size or need, not for permission**: the CD versions (66 to 230 MB, speech over USB
at under 1 MB/s) and Drascula's music add-ons (need Vorbis, MP3 or FLAC decoders
this build leaves out). Their licence files have not been read.

Excluded on principle: any commercial game, abandonware, and anything not on
ScummVM's own download site. No such game is offered.

## The terms in common

Each licence grants to whoever receives the archive ("You may"), and not only to
ScummVM. Six of the seven have these terms, in nearly these words:

1. Free distribution on any medium, with the readme or licence and all copyright
   notices and disclaimers intact.
2. A reasonable copying fee is allowed, as is aggregation in a larger, possibly
   commercial, distribution, "with proper attribution".
3. No fee for the game itself, and no reselling it as an individual item.
4. Modification is allowed, with a prominent notice in distributed modified
   versions. **Dreamweb's licence has no such clause: it grants no right to modify.**
5. A copyright statement. 6. A disclaimer of warranty.

Three of the readmes (Steel Sky, Queen, Drascula) add a non-binding preamble saying
that using the game "in things like commercial adventure game collections without
asking is just playing dirty".

How this project keeps to them:

- It doesn't distribute the games; `tools/fetch-games` fetches them, for you.
- Every file of each archive is kept, unmodified, licence included. The one
  difference from the archive is that a single wrapping directory (Queen's, Lure's)
  is removed, and a `SOURCE.txt` naming the URL and hash is added beside the files.
- Attribution is in the README, and in each licence file beside each game.
- Nothing here charges for anything.

A tree or stick made with the games in it, if you pass it on, is distribution by
you, and these terms apply to you.

## For the owner to look at

- **The licence files are ScummVM's copies.** The rights holders' own statements
  weren't independently found: ScummVM's wiki pages for these games refused
  automated fetching. The evidence here is the licence file in the release ScummVM
  distributes, which ScummVM's site offers as freeware. It isn't a statement seen
  from the rights holder directly.
- **Drascula's legal section is headed "DRASCULA adaptation for SCUMMVM."** Its
  clauses are still addressed to "You", and permit aggregation, so it is included;
  the heading could be read more narrowly.
- **Nippon Safes names two companies.** `readme.txt` gives the copyright as
  Dynabyte Software 1992 and the game's own `README` as "copyright '92 Euclidea srl".
  The licence is the former's text. Whether both are the same rights holder wasn't
  established.
- **The licence files carry no dates or signatures**, except the readmes' dates and
  the ScummVM team's introductions.
- **Copies of this data in any public tree or release** are not something this
  project does or needs. If that changes, read clause 2 again for attribution.

## Engine data

Three small files ScummVM itself ships (`dists/engine-data/`) are also needed:
`sky.cpt` for Steel Sky, `lure.dat` for Lure, `drascula.dat` for Drascula. They come
from the pinned ScummVM submodule, under its `COPYING` and `dists/engine-data/README`.
`tools/fetch-games` copies them beside the games, and they aren't in this repository
either. Queen's floppy version needs no `queen.tbl`.

## Screen sizes

Every included game runs at 320x200, read from the engine's `initGraphics` calls,
so it fits a 320x240 screen with 20-pixel bars, without scaling. Dreamweb's end
titles ask for 640x480 and fall back on 320x240, which the backend accepts. Engines
that need more were never built, and none of the seven games needs them.
