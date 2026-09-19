//! Entry point: catch_unwind around cli::run, exit code policy.

fn main() {
    let debug = std::env::var("SPARKLEBIOS_DEBUG").as_deref() == Ok("1");

    if !debug {
        std::panic::set_hook(Box::new(|_| {}));
    }

    let code = match std::panic::catch_unwind(sparklebios::cli::run) {
        Ok(code) => code,
        Err(_) => {
            if debug {
                eprintln!("bios: internal error");
            }
            0
        }
    };

    std::process::exit(code);
}
