use std::process;

// https://pastebin.com/emFNyUXe
// https://docs.rs/notify/4.0.15/notify/enum.DebouncedEvent.html
fn main() {
    let mut config = brickatlas::Config::new_from_args().unwrap_or_else(|e| {
        println!("error while configuring from command arguments: {}", e);
        process::exit(1);
    });
    if let Err(e) = brickatlas::run(&mut config) {
        println!("error while executing: {}", e);
        process::exit(1);
    }
}
