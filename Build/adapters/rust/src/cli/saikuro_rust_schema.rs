use std::fs;
use std::path::PathBuf;

use clap::Parser;
use serde_json::{json, Value};
use syn::{
    FnArg, GenericArgument, ImplItem, Item, Pat, PathArguments, ReturnType, Signature, TraitItem,
    Type,
};

#[derive(Debug, Parser)]
#[command(name = "saikuro-rust-schema")]
#[command(about = "Extract a Saikuro schema from a Rust source file")]
struct Args {
    #[arg(value_name = "SOURCE")]
    source: PathBuf,

    #[arg(long, default_value = "default")]
    namespace: String,

    #[arg(long)]
    pretty: bool,
}

fn primitive(name: &str) -> Value {
    json!({ "kind": "primitive", "type": name })
}

fn is_option_type(ty: &Type) -> bool {
    match ty {
        Type::Path(tp) => tp
            .path
            .segments
            .last()
            .map(|s| s.ident == "Option")
            .unwrap_or(false),
        Type::Reference(r) => is_option_type(&r.elem),
        _ => false,
    }
}

fn type_to_schema(ty: &Type) -> Value {
    match ty {
        Type::Reference(r) => type_to_schema(&r.elem),
        Type::Tuple(t) if t.elems.is_empty() => primitive("unit"),
        Type::ImplTrait(_) => json!({ "kind": "stream", "item": primitive("any") }),
        Type::Path(tp) => {
            let Some(seg) = tp.path.segments.last() else {
                return primitive("any");
            };
            let ident = seg.ident.to_string();

            match ident.as_str() {
                "bool" => primitive("bool"),
                "String" | "str" => primitive("string"),
                "f32" | "f64" => primitive("f64"),
                "i8" | "i16" | "i32" | "i64" | "isize" => primitive("i64"),
                "u8" | "u16" | "u32" | "u64" | "usize" => primitive("u64"),
                "Vec" => {
                    let item = first_type_arg(seg)
                        .map(type_to_schema)
                        .unwrap_or_else(|| primitive("any"));
                    json!({ "kind": "list", "item": item })
                }
                "Option" => {
                    let inner = first_type_arg(seg)
                        .map(type_to_schema)
                        .unwrap_or_else(|| primitive("any"));
                    json!({ "kind": "optional", "inner": inner })
                }
                "HashMap" | "BTreeMap" => {
                    let (key, value) = two_type_args(seg)
                        .map(|(k, v)| (type_to_schema(k), type_to_schema(v)))
                        .unwrap_or_else(|| (primitive("any"), primitive("any")));
                    json!({ "kind": "map", "key": key, "value": value })
                }
                "Result" => first_type_arg(seg)
                    .map(type_to_schema)
                    .unwrap_or_else(|| primitive("any")),
                other => json!({ "kind": "named", "name": other }),
            }
        }
        _ => primitive("any"),
    }
}

fn first_type_arg(seg: &syn::PathSegment) -> Option<&Type> {
    let PathArguments::AngleBracketed(ab) = &seg.arguments else {
        return None;
    };
    ab.args.iter().find_map(|arg| match arg {
        GenericArgument::Type(ty) => Some(ty),
        _ => None,
    })
}

fn two_type_args(seg: &syn::PathSegment) -> Option<(&Type, &Type)> {
    let PathArguments::AngleBracketed(ab) = &seg.arguments else {
        return None;
    };
    let mut it = ab.args.iter().filter_map(|arg| match arg {
        GenericArgument::Type(ty) => Some(ty),
        _ => None,
    });
    let first = it.next()?;
    let second = it.next()?;
    Some((first, second))
}

fn vis_to_schema(vis: &syn::Visibility) -> &'static str {
    match vis {
        syn::Visibility::Public(_) => "public",
        syn::Visibility::Inherited => "private",
        syn::Visibility::Restricted(restricted) => {
            let head = restricted
                .path
                .segments
                .first()
                .map(|s| s.ident.to_string())
                .unwrap_or_default();
            if restricted.in_token.is_none() && head == "crate" {
                "crate"
            } else if head == "crate" {
                "pub(crate)"
            } else if head == "super" {
                "pub(super)"
            } else {
                "private"
            }
        }
    }
}

fn function_schema(sig: &Signature, vis: Option<&syn::Visibility>) -> Value {
    let mut args = Vec::new();
    for (index, input) in sig.inputs.iter().enumerate() {
        let FnArg::Typed(typed) = input else {
            continue;
        };

        let name = match &*typed.pat {
            Pat::Ident(id) => id.ident.to_string(),
            _ => format!("arg{index}"),
        };

        let ty = type_to_schema(&typed.ty);
        let optional = is_option_type(&typed.ty);
        args.push(json!({
            "name": name,
            "type": ty,
            "optional": optional,
        }));
    }

    let returns = match &sig.output {
        ReturnType::Default => primitive("unit"),
        ReturnType::Type(_, ty) => type_to_schema(ty),
    };

    json!({
        "args": args,
        "returns": returns,
        "visibility": vis.map(vis_to_schema).unwrap_or("public"),
        "capabilities": [],
        "idempotent": false,
    })
}

fn self_type_name(ty: &Type) -> String {
    match ty {
        Type::Path(tp) => tp
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_else(|| "impl".to_owned()),
        _ => "impl".to_owned(),
    }
}

fn insert_function(functions: &mut serde_json::Map<String, Value>, key: String, schema: Value) {
    if !functions.contains_key(&key) {
        functions.insert(key, schema);
        return;
    }

    let mut n = 2usize;
    loop {
        let candidate = format!("{key}#{n}");
        if !functions.contains_key(&candidate) {
            eprintln!(
                "warning: duplicate extracted function name '{}', stored as '{}'",
                key, candidate
            );
            functions.insert(candidate, schema);
            break;
        }
        n += 1;
    }
}

fn extract_schema(source: &str, namespace: &str) -> Result<Value, String> {
    let file = syn::parse_file(source).map_err(|e| format!("failed to parse source: {e}"))?;

    let mut functions = serde_json::Map::new();

    for item in file.items {
        match item {
            Item::Fn(func) => {
                insert_function(
                    &mut functions,
                    func.sig.ident.to_string(),
                    function_schema(&func.sig, Some(&func.vis)),
                );
            }
            Item::Impl(impl_block) => {
                let ty_name = self_type_name(&impl_block.self_ty);
                let trait_name = impl_block
                    .trait_
                    .as_ref()
                    .and_then(|(_, path, _)| path.segments.last().map(|s| s.ident.to_string()));
                for impl_item in impl_block.items {
                    if let ImplItem::Fn(method) = impl_item {
                        let key = match &trait_name {
                            Some(name) => format!("{}::{}::{}", ty_name, name, method.sig.ident),
                            None => format!("{}::{}", ty_name, method.sig.ident),
                        };
                        insert_function(
                            &mut functions,
                            key,
                            function_schema(&method.sig, Some(&method.vis)),
                        );
                    }
                }
            }
            Item::Trait(trait_item) => {
                let trait_name = trait_item.ident.to_string();
                for trait_member in trait_item.items {
                    if let TraitItem::Fn(method) = trait_member {
                        let key = format!("{}::{}", trait_name, method.sig.ident);
                        insert_function(&mut functions, key, function_schema(&method.sig, None));
                    }
                }
            }
            _ => {}
        }
    }

    Ok(json!({
        "version": 1,
        "namespaces": {
            (namespace): {
                "functions": functions
            }
        },
        "types": {}
    }))
}

fn main() {
    let args = Args::parse();

    let source = match fs::read_to_string(&args.source) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to read {}: {e}", args.source.display());
            std::process::exit(1);
        }
    };

    let schema = match extract_schema(&source, &args.namespace) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };

    let out = if args.pretty {
        serde_json::to_string_pretty(&schema).expect("serialize schema")
    } else {
        serde_json::to_string(&schema).expect("serialize schema")
    };

    println!("{out}");
}
