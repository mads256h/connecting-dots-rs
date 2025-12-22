use connecting_dots_rs::run;
use clap::Parser;



#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Path to background image
    #[arg(short, long)]
    background_image: Option<String>,

    /// Window class
    #[arg(short, long, default_value="connecting-dots")]
    class: String,
}


fn main() -> anyhow::Result<()> {
    let args = Args::try_parse()?;

    run(args.background_image, args.class)?;

    Ok(())
}

