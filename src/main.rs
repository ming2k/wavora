fn main() {
    if let Some(argument) = std::env::args_os().nth(1) {
        if argument == "--version" || argument == "-V" {
            println!("wavora {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        if argument == "--help" || argument == "-h" {
            println!(
                "Wavora {}\n\nUsage: wavora [AUDIO_FILE | MUSIC_DIRECTORY]...\n\n  -h, --help       Show this help\n  -V, --version    Show the version",
                env!("CARGO_PKG_VERSION")
            );
            return;
        }
    }
    if let Err(error) = wavora::app::run() {
        eprintln!("Wavora failed to start: {error}");
        std::process::exit(1);
    }
}
