ui_lang::include_app!("src/ui/app.ice");

mod backend;
mod call;
mod editor;

fn main() -> iced::Result {
    Ducktape::run()
}

#[cfg(test)]
mod tests;
