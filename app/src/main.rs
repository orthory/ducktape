ui_lang::include_app!("src/ui/app.ice");

mod backend;
mod call;
mod video;
mod editor;
mod pages;

fn main() -> iced::Result {
    Ducktape::run()
}

// The app is a bin crate: `app/tests/` cannot see `Ducktape`, so the frame
// probe lives in the crate beside the suite it gates.
#[cfg(test)]
mod frame_probe;
#[cfg(test)]
mod tests;
