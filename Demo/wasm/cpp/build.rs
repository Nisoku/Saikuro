fn main() {
    println!("cargo:rerun-if-changed=insight_cpp.cpp");
    println!("cargo:rerun-if-changed=../../../Build/adapters/c/include/saikuro.h");

    let target = std::env::var("TARGET").unwrap_or_default();
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let emsdk_root = std::env::var("EMSDK")
        .or_else(|_| std::env::var("HOME").map(|h| format!("{}/.emsdk", h)))
        .or_else(|_| std::env::var("USERPROFILE").map(|h| format!("{}/.emsdk", h)))
        .expect("EMSDK environment variable not set and could not determine home directory");

    let include_dir = format!("{}/../../../Build/adapters/c/include", manifest_dir);

    let mut build = cc::Build::new();
    build.file("insight_cpp.cpp");
    build.cpp(true);
    build.include(&include_dir);

    if target.contains("wasm32") {
        let emsdk_clangxx = format!("{}/upstream/bin/clang++", emsdk_root);
        build.compiler(&emsdk_clangxx);
        build.flag("--target=wasm32-unknown-unknown");
        build.flag("-nostdlib");
        build.flag("-fno-builtin");
        build.flag("-std=c++17");
        build.cpp_link_stdlib(None);
    }

    build.compile("insight_cpp");
}
