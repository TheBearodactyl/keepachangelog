use clap::Parser;

mod changelog;
mod flow;
mod hook;

#[macro_export]
macro_rules! validation_error {
    ($msg:expr) => {
        Ok(bearask::Validation::Invalid(bearask::ErrorMessage::Custom(
            $msg.into(),
        )))
    };
}

#[derive(Parser)]
#[command(name = "keepachangelog", version, about)]
struct Cli {
    #[arg(short, long, default_value = "CHANGELOG.md")]
    file: String,

    #[arg(long)]
    setup_hook: bool,
}

fn main() -> miette::Result<()> {
    let cli = Cli::parse();

    if cli.setup_hook {
        return hook::install(&cli.file);
    }

    hook::reopen_tty();
    flow::run(&cli.file)
}
