//! Asyncify instrumentation for wasm-pack output.
//!
//! A manifest opts in with:
//!
//! ```toml
//! [package.metadata.saikuro.asyncify]
//! entry = "asyncify_entry"
//! ```
//!

use std::path::Path;

use anyhow::{bail, Context};
use serde::Deserialize;

use crate::paths;
use crate::run;
use crate::wasmopt;

/// Cargo manifest section that opts a package into Asyncify instrumentation.
const MANIFEST_KEY: &str = "saikuro";
const ASYNCIFY_KEY: &str = "asyncify";

/// File name of the generated shim inside the wasm-pack output directory.
const SHIM_FILE: &str = "saikuro_asyncify.js";

/// Bare specifier the wasm module imports `asyncify_suspend` from. Must match
/// the `#[wasm_bindgen(module = "...")]` in `saikuro-exec`'s asyncify engine.
const IMPORT_SPECIFIER: &str = "saikuro";

/// Host driver that loads the instrumented module and runs the suite.
const HOST_FILE: &str = "asyncify_host.mjs";

/// Suite runs per gate invocation
const E2E_REPETITIONS: &str = "10";

/// Marks a glue file as already instrumented, so a stale output directory is
/// reported instead of silently patched twice.
const EPILOGUE_MARKER: &str = "saikuro asyncify epilogue";

/// Export `wasm-opt --asyncify` must add. Checked after the pass so a
/// misconfigured or silently-skipped optimization fails the build.
const INSTRUMENTED_EXPORT: &str = "asyncify_start_unwind";

/// Forces the reference-spill path.
const REF_TYPES_FIXTURE: &str = "asyncify_ref_types.wat";

/// One per heap type, since the spill tables are keyed by heap type.
const REF_SPILL_TABLES: [&str; 2] = ["__asyncify_ref_table_extern", "__asyncify_ref_table_func"];

/// The `#[package.metadata.saikuro.asyncify]` table.
#[derive(Deserialize)]
struct AsyncifyConfig {
    /// Name of the synchronous `#[wasm_bindgen]` export that blocks in
    /// `block_on`. Driven through the generated `callSync`.
    entry: String,
}

/// Read the opt-in for `manifest`, or `None` when the package is not opted in.
fn read_config(manifest: &Path) -> anyhow::Result<Option<AsyncifyConfig>> {
    let text = std::fs::read_to_string(manifest)
        .with_context(|| format!("read {}", manifest.display()))?;
    let value: toml::Value =
        toml::from_str(&text).with_context(|| format!("parse {}", manifest.display()))?;

    let Some(package) = value.get("package") else {
        return Ok(None);
    };
    let Some(saikuro) = package.get("metadata").and_then(|m| m.get(MANIFEST_KEY)) else {
        return Ok(None);
    };
    let Some(asyncify) = saikuro.get(ASYNCIFY_KEY) else {
        return Ok(None);
    };

    let config: AsyncifyConfig = asyncify.clone().try_into().with_context(|| {
        format!(
            "parse [package.metadata.{MANIFEST_KEY}.{ASYNCIFY_KEY}] in {}; \
                 it must set `entry` to the blocking #[wasm_bindgen] export",
            manifest.display()
        )
    })?;
    Ok(Some(config))
}

/// Instrument the wasm-pack output in `out_dir` for Asyncify. A no-op unless
/// `manifest` opts in.
pub fn instrument(manifest: &Path, out_dir: &Path) -> anyhow::Result<()> {
    let Some(config) = read_config(manifest)? else {
        return Ok(());
    };
    let stem = package_stem(out_dir)?;

    let wasm = out_dir.join(format!("{stem}_bg.wasm"));
    let glue = out_dir.join(format!("{stem}.js"));
    for path in [&wasm, &glue] {
        if !path.is_file() {
            bail!(
                "asyncify: expected {} to exist; the package name in {} \
                 does not match the built artifact",
                path.display(),
                out_dir.join("package.json").display()
            );
        }
    }

    run_wasm_opt(&wasm)?;
    verify_instrumented(&wasm)?;
    write_shim(out_dir)?;
    patch_glue(&glue, &config.entry)?;
    println!("  asyncify  instrumented {stem} (entry `{}`)", config.entry);
    Ok(())
}

/// Run the vendored `wasm-opt --asyncify` pass over `wasm` in place.
fn run_wasm_opt(wasm: &Path) -> anyhow::Result<()> {
    let wasm_opt = wasmopt::ensure().context("locate the vendored wasm-opt")?;
    let parent = wasm.parent().context("wasm path has no parent")?;
    // Binaryen cannot read and write the same file, so stage through a sibling
    // temp file and move it into place only on success.
    let staged = parent.join(format!(
        ".{}.asyncify-staged",
        wasm.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("module")
    ));
    run::run(
        &paths::repo_root(),
        wasm_opt.to_string_lossy().as_ref(),
        [
            wasm.to_string_lossy().as_ref(),
            "--asyncify",
            "-g",
            "-o",
            staged.to_string_lossy().as_ref(),
        ],
    )
    .with_context(|| "wasm-opt --asyncify")?;
    std::fs::rename(&staged, wasm).with_context(|| {
        format!(
            "move instrumented {} over {}",
            staged.display(),
            wasm.display()
        )
    })?;
    Ok(())
}

/// Assert the vendored `wasm-opt` carries the reference-spill fix.
pub fn verify_reference_spill_support() -> anyhow::Result<()> {
    let wasm_opt = wasmopt::ensure().context("locate the vendored wasm-opt")?;
    let fixture = paths::xtask_assets_dir().join(REF_TYPES_FIXTURE);
    let instrumented = fixture.with_extension("instrumented.wat");

    run::run(
        &paths::repo_root(),
        wasm_opt.to_string_lossy().as_ref(),
        [
            fixture.to_string_lossy().as_ref(),
            "--asyncify",
            "--enable-reference-types",
            "--enable-gc",
            "-S",
            "-o",
            instrumented.to_string_lossy().as_ref(),
        ],
    )
    .with_context(|| {
        format!(
            "{}: This version of wasm-opt cannot work with reference-typed locals. \
             please check if you are on the latest version.",
            fixture.display()
        )
    })?;

    let text = std::fs::read_to_string(&instrumented)
        .with_context(|| format!("read {}", instrumented.display()))?;
    std::fs::remove_file(&instrumented)
        .with_context(|| format!("remove {}", instrumented.display()))?;

    for table in REF_SPILL_TABLES {
        if !text.contains(table) {
            bail!(
                "asyncify: instrumented fixture has no `{table}`. wasm-opt ran but \
                 did not spill reference-typed locals, so it appears \
                 to be an older version."
            );
        }
    }
    Ok(())
}

/// Fail unless the Asyncify pass actually rewrote the module.
fn verify_instrumented(wasm: &Path) -> anyhow::Result<()> {
    let bytes = std::fs::read(wasm).with_context(|| format!("read {}", wasm.display()))?;
    if !bytes
        .windows(INSTRUMENTED_EXPORT.len())
        .any(|w| w == INSTRUMENTED_EXPORT.as_bytes())
    {
        bail!(
            "asyncify: {} has no `{INSTRUMENTED_EXPORT}` export after the pass; \
             the module was not instrumented. Check that the manifest enables the \
             `asyncify` feature on saikuro-exec.",
            wasm.display()
        );
    }
    Ok(())
}

/// Emit the host shim plus the bare-specifier package wasm imports from.
fn write_shim(out_dir: &Path) -> anyhow::Result<()> {
    let asset = paths::xtask_assets_dir().join(SHIM_FILE);
    let shim =
        std::fs::read(&asset).with_context(|| format!("read shim asset {}", asset.display()))?;
    std::fs::write(out_dir.join(SHIM_FILE), &shim)
        .with_context(|| format!("write {}", out_dir.join(SHIM_FILE).display()))?;

    let pkg_dir = out_dir.join("node_modules").join(IMPORT_SPECIFIER);
    std::fs::create_dir_all(&pkg_dir).with_context(|| format!("mkdir {}", pkg_dir.display()))?;
    std::fs::write(
        pkg_dir.join("package.json"),
        format!(
            "{{\n  \"name\": \"{IMPORT_SPECIFIER}\",\n  \"version\": \"0.0.0\",\n  \
             \"type\": \"module\",\n  \"main\": \"index.js\",\n  \
             \"exports\": \"./index.js\"\n}}\n"
        ),
    )
    .with_context(|| format!("write {}", pkg_dir.join("package.json").display()))?;
    // Re-export rather than copy, so there is exactly one shim implementation.
    std::fs::write(
        pkg_dir.join("index.js"),
        format!("export * from \"../../{SHIM_FILE}\";\n"),
    )
    .with_context(|| format!("write {}", pkg_dir.join("index.js").display()))?;
    Ok(())
}

/// Append the `callSync` epilogue to the generated glue.
fn patch_glue(glue: &Path, entry: &str) -> anyhow::Result<()> {
    let source =
        std::fs::read_to_string(glue).with_context(|| format!("read {}", glue.display()))?;

    if source.contains(EPILOGUE_MARKER) {
        bail!(
            "asyncify: {} is already instrumented; remove the stale wasm-pack \
             output directory and rebuild",
            glue.display()
        );
    }

    const EXPORT_LINE: &str = "export { initSync, __wbg_init as default };";
    if !source.contains(EXPORT_LINE) {
        bail!(
            "asyncify: {} does not contain `{EXPORT_LINE}`; the wasm-bindgen web \
             glue shape changed and the asyncify epilogue can no longer be applied \
             safely",
            glue.display()
        );
    }
    if !source.contains(&format!("export function {entry}(")) {
        bail!(
            "asyncify: entry `{entry}` is not exported as a top-level function in \
             {}; the configured [package.metadata.saikuro.asyncify] entry is wrong \
             or the export signature changed",
            glue.display()
        );
    }

    let epilogue = format!(
        r#"
// --- {marker} (generated by xtask; do not edit) ---
import {{ createAsyncify }} from "./{shim}";
const __saikuro_asyncify = createAsyncify({entry});
const __saikuro_initSync = initSync;
initSync = function (module_or_path) {{
    const instance = __saikuro_initSync(module_or_path);
    __saikuro_asyncify.attach(instance);
    return instance;
}};
const __saikuro_wbg_init = __wbg_init;
__wbg_init = async function (module_or_path) {{
    const instance = await __saikuro_wbg_init(module_or_path);
    __saikuro_asyncify.attach(instance);
    return instance;
}};
export const callSync = (op) => __saikuro_asyncify.callSync(op);
"#,
        marker = EPILOGUE_MARKER,
        shim = SHIM_FILE,
    );

    std::fs::write(glue, format!("{source}{epilogue}"))
        .with_context(|| format!("write {}", glue.display()))?;
    Ok(())
}

/// The wasm-pack artifact stem
fn package_stem(out_dir: &Path) -> anyhow::Result<String> {
    let manifest = out_dir.join("package.json");
    let text = std::fs::read_to_string(&manifest)
        .with_context(|| format!("read {}", manifest.display()))?;
    let value: toml::Value =
        serde_json::from_str(&text).with_context(|| format!("parse {}", manifest.display()))?;
    let name = value
        .get("name")
        .and_then(toml::Value::as_str)
        .with_context(|| format!("{} has no string `name`", manifest.display()))?;
    Ok(name.replace('-', "_"))
}

/// Run `wasm-pack` for `src` into `out_dir`, then instrument the output.
pub fn wasm_pack(src: &Path, out_dir: &Path, extra_features: &str) -> anyhow::Result<()> {
    let manifest = src.join("Cargo.toml");
    // Stale glue
    if out_dir.is_dir() {
        std::fs::remove_dir_all(out_dir).with_context(|| format!("clear {}", out_dir.display()))?;
    }
    std::fs::create_dir_all(out_dir).with_context(|| format!("mkdir {}", out_dir.display()))?;

    let name = src
        .file_name()
        .and_then(|n| n.to_str())
        .with_context(|| format!("{} has no directory name", src.display()))?;

    run::run(
        &paths::repo_root(),
        "wasm-pack",
        [
            "build",
            name,
            "--target",
            "web",
            "--out-dir",
            &out_dir.display().to_string(),
            "--no-default-features",
            "--features",
            extra_features,
        ],
    )
    .with_context(|| format!("wasm-pack {name}"))?;

    let gitignore = out_dir.join(".gitignore");
    if gitignore.is_file() {
        std::fs::remove_file(&gitignore)
            .with_context(|| format!("remove {}", gitignore.display()))?;
    }
    instrument(&manifest, out_dir)
}

/// Build the test suite and drive it from Node.
pub fn test_e2e() -> anyhow::Result<()> {
    let root = paths::repo_root();
    let out_dir = root.join("target").join("asyncify-e2e");
    verify_reference_spill_support()?;
    wasm_pack(&paths::tests_dir(), &out_dir, "wasm,asyncify")?;

    let host = paths::xtask_assets_dir().join(HOST_FILE);
    run::run(
        &root,
        "node",
        [
            &host.display().to_string(),
            &out_dir.display().to_string(),
            E2E_REPETITIONS,
        ],
    )
    .context("asyncify e2e host")
}
