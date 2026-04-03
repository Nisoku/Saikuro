#![cfg(feature = "cli")]

use clap::Parser;

#[derive(Debug, Parser)]
#[command(author, version, about = "Saikuro binding generator (minimal CLI)")]
struct Opts {
    /// Path to input schema JSON file
    #[arg(long)]
    schema: String,

    /// Target language (typescript, python, csharp)
    #[arg(long)]
    lang: String,

    /// Output directory
    #[arg(long)]
    out: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let opts = Opts::parse();

    // TODO: implement the actual codegen logic here. For now, just print the options.
    if opts.schema.is_empty() {
        eprintln!("--schema is required");
        std::process::exit(2);
    }

    println!(
        "saikuro-codegen: would generate lang={} from schema={} -> {:?}",
        opts.lang, opts.schema, opts.out
    );

    Ok(())
}
