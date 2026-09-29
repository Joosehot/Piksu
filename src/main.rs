use anyhow::bail;
use clap::Parser;
use piksu::config::Config;
use std::path::PathBuf;

/// Piksu: describe a sprite, get pixel art and animation frames.
///
///   piksu "a small green slime with big eyes, bouncing"
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// The sentence.
    sentence: Option<String>,
    /// Output folder (default: out/<subject>_<animation>).
    #[arg(short, long)]
    out: Option<PathBuf>,
    /// Print tokens, every finalist, judges and checks.
    #[arg(long)]
    explain: bool,
    /// Skip unknown words instead of failing.
    #[arg(long)]
    lenient: bool,
    /// Use this rules.toml instead of the built-in one.
    #[arg(long)]
    rules: Option<PathBuf>,
    /// List every word Piksu knows.
    #[arg(long)]
    vocabulary: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    if cli.vocabulary {
        for w in piksu::lexicon::vocabulary() {
            println!("{w}");
        }
        return Ok(());
    }
    let Some(sentence) = cli.sentence else { bail!("give a sentence, e.g. piksu \"a green slime, bouncing\"") };
    let cfg = Config::load(cli.rules.as_deref())?;
    match piksu::make(&sentence, &cfg, &piksu::Options { lenient: cli.lenient }) {
        Ok(sp) => {
            if cli.explain {
                eprintln!("{}", sp.explain);
            }
            let dir = cli.out.unwrap_or_else(|| PathBuf::from("out").join(piksu::output::slug(&sp)));
            piksu::output::write(&sp, &cfg, &dir)?;
            let frames: usize = sp.anims.iter().map(|a| a.1.len()).sum();
            println!("{} -> {} ({}px, {} frame{})", sentence, dir.display(), sp.size, frames, if frames == 1 { "" } else { "s" });
            print!("{}", piksu::output::why_text(&sp).lines().skip(2).map(|l| format!("  {l}\n")).collect::<String>());
            Ok(())
        }
        Err(diags) => {
            for d in &diags {
                eprintln!("{d}");
            }
            bail!("no sprite made")
        }
    }
}
