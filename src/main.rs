//! Themezzz — Telegram theme-creator bot, full Rust port.
//! Thin binary wrapper: all logic lives in the library.

use themezzz::run_bot;

fn main() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime")
        .block_on(run_bot());
}
