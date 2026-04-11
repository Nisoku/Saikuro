use std::fs;
use std::path::PathBuf;

use clap::Parser;
use regex::Regex;
use serde_json::{json, Value};

#[derive(Debug, Parser)]
#[command(name = "saikuro-c-schema")]
#[command(about = "Extract a Saikuro schema from a C header")]
struct Args {
    #[arg(value_name = "HEADER")]
    header: PathBuf,

    #[arg(long, default_value = "default")]
    namespace: String,

    #[arg(long)]
    pretty: bool,
}

fn primitive(name: &str) -> Value {
    json!({ "kind": "primitive", "type": name })
}

fn map_c_type(raw: &str) -> Value {
    let normalized = raw
        .replace("const", "")
        .replace("volatile", "")
        .replace("  ", " ")
        .trim()
        .to_string();

    if normalized.contains("char*") || normalized.contains("char *") {
        return primitive("string");
    }
    if normalized.contains("bool") {
        return primitive("bool");
    }
    if normalized.contains("float") || normalized.contains("double") {
        return primitive("f64");
    }
    if normalized == "void" {
        return primitive("unit");
    }
    if normalized.contains("int") || normalized.contains("long") || normalized.contains("size_t") {
        return primitive("i64");
    }
    primitive("any")
}

fn parse_arg(arg: &str) -> Option<(String, Value, bool)> {
    let arg = arg.trim();
    if arg.is_empty() || arg == "void" {
        return None;
    }

    let mut parts: Vec<&str> = arg.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    let name = parts
        .pop()
        .unwrap_or("arg")
        .trim()
        .trim_matches('*')
        .to_string();
    let ty = parts.join(" ");
    let schema_ty = map_c_type(&format!(
        "{}{}",
        ty,
        if arg.contains('*') { "*" } else { "" }
    ));
    Some((name, schema_ty, false))
}

fn extract_schema(source: &str, namespace: &str) -> Value {
    let proto = Regex::new(
        r"^\s*([A-Za-z_][A-Za-z0-9_\s\*]+?)\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(([^)]*)\)\s*;",
    )
    .expect("compile regex");

    let mut functions = serde_json::Map::new();

    for line in source.lines() {
        let Some(cap) = proto.captures(line) else {
            continue;
        };

        let ret_ty = cap.get(1).map(|m| m.as_str()).unwrap_or("void");
        let name = cap.get(2).map(|m| m.as_str()).unwrap_or("unknown");
        let args_raw = cap.get(3).map(|m| m.as_str()).unwrap_or("");

        if name.starts_with("saikuro_") {
            continue;
        }

        let args = args_raw
            .split(',')
            .filter_map(parse_arg)
            .map(|(n, t, optional)| {
                json!({
                    "name": n,
                    "type": t,
                    "optional": optional,
                })
            })
            .collect::<Vec<_>>();

        functions.insert(
            name.to_owned(),
            json!({
                "args": args,
                "returns": map_c_type(ret_ty),
                "visibility": "public",
                "capabilities": [],
                "idempotent": false,
            }),
        );
    }

    json!({
        "version": 1,
        "namespaces": {
            (namespace): {
                "functions": functions
            }
        },
        "types": {}
    })
}

fn main() {
    let args = Args::parse();

    let source = match fs::read_to_string(&args.header) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to read {}: {e}", args.header.display());
            std::process::exit(1);
        }
    };

    let schema = extract_schema(&source, &args.namespace);
    let out = if args.pretty {
        serde_json::to_string_pretty(&schema).expect("serialize schema")
    } else {
        serde_json::to_string(&schema).expect("serialize schema")
    };

    println!("{out}");
}
