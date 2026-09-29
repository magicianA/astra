use anyhow::{Result, ensure};
fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or_else(|| {
        anyhow::anyhow!("Usage: astra_atmosphere OUTPUT_DIRECTORY [ORDERS=8] [--validation]")
    })?;
    if path == "--help" {
        println!("astra_atmosphere OUTPUT_DIRECTORY [ORDERS=8] [--validation]");
        return Ok(());
    }
    let mut orders = 8;
    let mut validation = false;
    for arg in args {
        if arg == "--validation" {
            validation = true;
        } else {
            orders = arg.parse()?;
        }
    }
    let gpu = astro::gpu::Gpu::new(validation)?;
    println!("{}", gpu.name);
    for (preset, name) in ["clear.bin", "hazy.bin"].iter().enumerate() {
        astro::atmosphere::AtmosphereLut::generate(
            &gpu,
            &std::path::Path::new(&path).join(name),
            preset as u32,
            orders,
        )?;
    }
    println!("Validation errors: {}", gpu.errors());
    ensure!(gpu.errors() == 0, "Vulkan validation errors");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("Astra: {e:#}");
        std::process::exit(1);
    }
}
