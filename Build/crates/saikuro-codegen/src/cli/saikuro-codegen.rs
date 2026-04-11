#![cfg(feature = "cli")]

use std::path::Path;
use std::{fs, path::PathBuf};

use clap::Parser;
use saikuro_codegen::{
    c::CGenerator, cpp::CppGenerator, csharp::CSharpGenerator, generator::BindingGenerator,
    python::PythonGenerator, rust::RustGenerator, typescript::TypeScriptGenerator,
};
use saikuro_core::schema::Schema;

// TODO: This is a minimal CLI for manual testing.
// We should build out a more robust CLI with subcommands, better error handling, etc.
// in the future tho
#[derive(Debug, Parser)]
#[command(author, version, about = "Saikuro binding generator (minimal CLI)")]
struct Opts {
    /// Path to input schema JSON file
    #[arg(long)]
    schema: String,

    /// Target language (typescript, python, csharp, c, cpp, rust)
    #[arg(long)]
    lang: String,

    /// Output directory
    #[arg(long)]
    out: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let opts = Opts::parse();

    let schema_json = fs::read_to_string(&opts.schema)?;
    let schema: Schema = serde_json::from_str(&schema_json)?;

    let generator: Box<dyn BindingGenerator> = match opts.lang.as_str() {
        "python" => Box::new(PythonGenerator),
        "typescript" | "ts" => Box::new(TypeScriptGenerator),
        "csharp" | "cs" => Box::new(CSharpGenerator),
        "c" => Box::new(CGenerator),
        "cpp" | "cxx" | "c++" => Box::new(CppGenerator),
        "rust" | "rs" => Box::new(RustGenerator),
        other => anyhow::bail!(
            "unsupported language '{other}'. expected one of: python, typescript|ts, csharp|cs, c, cpp|cxx|c++, rust|rs"
        ),
    };

    let output = generator.generate(&schema)?;

    let out_dir = opts
        .out
        .map(PathBuf::from)
        .unwrap_or(std::env::current_dir()?);
    fs::create_dir_all(&out_dir)?;

    for file in output.files {
        // Validate file.path is not absolute and does not contain ParentDir or RootDir
        let rel_path = &file.path;
        let rel_path_obj = Path::new(rel_path);
        if rel_path_obj.is_absolute()
            || rel_path_obj.components().any(|c| {
                matches!(
                    c,
                    std::path::Component::ParentDir
                        | std::path::Component::RootDir
                        | std::path::Component::Prefix(_)
                )
            })
        {
            anyhow::bail!("output file path '{rel_path}' is not allowed (absolute or contains parent/root dir)");
        }
        let path = out_dir.join(rel_path_obj);
        // Ensure the resulting path is inside out_dir
        let canon_out_dir = out_dir.canonicalize()?;
        let canon_path = if path.exists() {
            path.canonicalize()?
        } else if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
            parent.canonicalize()?.join(
                path.file_name()
                    .expect("file_name should exist after filtering ParentDir/RootDir"),
            )
        } else {
            // No parent or empty parent means file is directly in out_dir
            fs::create_dir_all(&out_dir)?;
            let canon_out_dir = out_dir.canonicalize()?;
            canon_out_dir.join(
                path.file_name()
                    .expect("file_name should exist after filtering ParentDir/RootDir"),
            )
        };
        if !canon_path.starts_with(&canon_out_dir) {
            anyhow::bail!(
                "output file path '{:?}' escapes output directory",
                canon_path
            );
        }
        fs::write(&path, file.content)?;
    }

    Ok(())
}
