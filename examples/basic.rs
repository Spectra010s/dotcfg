//! Basic dotcfg example
//! Run with: `cargo run --example basic`

use dotcfg::DotCfg;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default)]
struct AppConfig {
    theme: String,
    language: String,
    notifications: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use ~/.myapp/config.toml (default dot strategy)
    let cfg = DotCfg::new("myapp");

    // Save a default config if none exists
    let config: AppConfig = cfg.load_or_default()?;
    println!("Current config: {config:#?}");
    println!("Config file: {:?}", cfg.file_path()?);

    // Update a single key without reloading whole file
    cfg.set("theme", "dark")?;
    println!("theme = {}", cfg.get("theme")?);

    // Show XDG alternative (commented — would use ~/.config/myapp/config.toml)
    // let xdg_cfg = DotCfg::new("myapp").xdg();

    Ok(())
}
