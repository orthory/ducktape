ui_lang::include_app!("src/ui/app.ice");

mod backend;
mod call;
mod video;
mod editor;
mod pages;

fn main() -> iced::Result {
    install_log();
    // macOS launches a GUI with a 256-fd soft limit; the app's own stores,
    // sockets and the node it hosts hit that as a bare EMFILE. raised AFTER
    // install_log so the outcome lands in app.log like everything else.
    match node::resource_limits::raise_open_file_limit() {
        Ok(soft_limit) => tracing::info!(target: "ducktape::app", soft_limit, "open-file limit"),
        Err(err) => tracing::warn!(
            target: "ducktape::app",
            reason = "open_file_limit_unraised",
            error = %err,
            "open-file limit left at the inherited default"
        ),
    }
    Ducktape::run()
}

/// the app's own sink: `<DUCKTAPE_HOME or ~/.ducktape>/app.log`, rotated at
/// open exactly like the node's `daemon.log`, plus a panic hook that lands in
/// it. the FILE ONLY — a GUI's stderr is `/dev/null` under every launcher a
/// member actually uses, and iced's own prints are all a terminal should see.
///
/// `RUST_LOG` ADDS to `info` rather than replacing it, the `noded::log` rule:
/// `RUST_LOG=ducktape::auth=debug` turns one plane up without turning the rest
/// off. a malformed value falls back to `info` — strictly, not lossily, because
/// the lossy parser reports each skipped directive on stderr.
///
/// no home, no file: the events go where they went before (nowhere), rather
/// than a GUI refusing to start over a log it could not open.
fn install_log() {
    use tracing_subscriber::layer::SubscriberExt as _;
    use tracing_subscriber::util::SubscriberInitExt as _;
    use tracing_subscriber::{EnvFilter, Layer as _};

    let env = std::env::var("RUST_LOG").unwrap_or_default();
    let filter = EnvFilter::builder()
        .parse(format!("info,{env}"))
        .unwrap_or_else(|_| EnvFilter::new("info"));
    let file = backend::duck_home()
        .ok()
        .and_then(|home| node::log_file::open_rotating(&home.join("app.log")).ok());
    let file_layer = file.map(|file| {
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(std::sync::Mutex::new(file))
    });
    let _ = tracing_subscriber::registry()
        .with(file_layer.with_filter(filter))
        .try_init();
    install_panic_hook();
}

/// chain, don't replace: the default hook keeps the backtrace on whatever
/// stderr there is; this one puts the payload and location where a member can
/// find them after the window is gone.
fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|at| format!("{}:{}:{}", at.file(), at.line(), at.column()))
            .unwrap_or_default();
        tracing::error!(
            target: "ducktape::app",
            event = "app_panic",
            thread = std::thread::current().name().unwrap_or("?"),
            payload = info.payload_as_str().unwrap_or("non-string panic payload"),
            location,
            "panicked at: {info}"
        );
        default(info);
    }));
}

// The app is a bin crate: `app/tests/` cannot see `Ducktape`, so the frame
// probe lives in the crate beside the suite it gates.
#[cfg(test)]
mod frame_probe;
#[cfg(test)]
mod tests;
