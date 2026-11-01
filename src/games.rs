//! The games the list offers. docs/games.md says why each is here;
//! tools/fetch-games fetches them to games/<id>/ in the app's directory.

use std::path::{Path, PathBuf};

pub struct Game {
    /// ScummVM's game id, and its directory under games/.
    pub id: &'static str,
    /// As the list shows it: 18 characters at most.
    pub title: &'static str,
    pub detail: &'static str,
}

pub const GAMES: &[Game] = &[
    Game { id: "sky", title: "STEEL SKY", detail: "Beneath a Steel Sky (floppy)" },
    Game { id: "queen", title: "AMAZON QUEEN", detail: "Flight of the Amazon Queen (floppy)" },
    Game { id: "lure", title: "LURE", detail: "Lure of the Temptress" },
    Game { id: "drascula", title: "DRASCULA", detail: "Drascula: The Vampire Strikes Back" },
    Game { id: "dreamweb", title: "DREAMWEB", detail: "Dreamweb (floppy, needs the keys)" },
    Game { id: "soltys", title: "SOLTYS", detail: "Soltys" },
    Game { id: "nippon", title: "NIPPON SAFES", detail: "Nippon Safes, Inc. (needs the keys)" },
];

pub fn find(id: &str) -> Option<&'static Game> {
    GAMES.iter().find(|g| g.id == id)
}

impl Game {
    pub fn dir(&self, app: &Path) -> PathBuf {
        app.join("games").join(self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_fit_the_list() {
        for g in GAMES {
            assert!(g.title.len() <= 18, "{}", g.title);
            assert!(g.detail.len() <= 40, "{}", g.detail);
        }
    }
}
