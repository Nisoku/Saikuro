//! Saikuro Runtime Server (native binary)

fn main() -> anyhow::Result<()> {
    saikuro_runtime::native::run()
}
