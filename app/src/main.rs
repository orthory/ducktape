ui_lang::include_app!("src/ui/app.ice");

mod backend;

fn main() -> iced::Result {
    Ducktape::run()
}
