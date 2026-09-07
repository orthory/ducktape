//! The Members roster as a module-owned view: who may act on this network,
//! drawn from the rows the desktop app pushes. A row opens its record; the
//! record's writes — copy the node key, pause or resume an agent, open a
//! membership ballot — leave as intents the app signs.

pub mod host;

ui_lang::include_app!("src/ui/app.ice");

ui_lang_guest::export_app!(
    MembersView,
    "Members",
    "Who may act on this network: validators, residents and registered agents.",
    ["members"]
);
