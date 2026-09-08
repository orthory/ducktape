fn main() {
    ui_lang_build::compile_dir_for("src/ui", ui_lang_build::Target::Tree)
        .expect("compile the node view's Ice sources");
}
