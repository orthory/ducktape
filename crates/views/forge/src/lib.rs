//! Forge as a module-owned view: the repo overview, one repo's code, pull
//! requests and issues, and an item's merge box, reviews and discussion,
//! drawn from the facts the desktop app pushes. Every act — opening a repo
//! or an item, a directory or file pick, a review with its staged line
//! comments, a merge — leaves as an intent the app signs.

pub mod host;

ui_lang::include_app!("src/ui/app.ice");

ui_lang_guest::export_app!(
    ForgeView,
    "Forge",
    "This workspace's repositories: their code, pull requests and issues, with reviews and merges.",
    ["forge"]
);
