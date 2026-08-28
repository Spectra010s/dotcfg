//! Example demonstrating how to integrate `dotcfg` with `clap`.
//!
//! Demonstrates the common CLI configuration pattern:
//!   **CLI argument flag > Config file on disk > Default fallback**
//!
//! Run with:
//!   cargo run --example clap_cli -- --help
//!   cargo run --example clap_cli -- --host 0.0.0.0 --port 9090
//!   cargo run --example clap_cli -- --save-default

use clap::Parser;
use dotcfg::DotCfg;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
struct AppConfig {
    host: String,
    port: u16,
    log_level: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 8080,
            log_level: "info".into(),
        }
    }
}

/// A sample CLI tool demonstrating dotcfg + clap configuration precedence.
#[derive(Parser, Debug)]
#[command(name = "mycli", about = "CLI with dotcfg + clap precedence")]
struct Cli {
    /// Server host (overrides config file)
    #[arg(short = 'H', long)]
    host: Option<String>,

    /// Server port (overrides config file)
    #[arg(short, long)]
    port: Option<u16>,

    /// Log level (overrides config file)
    #[arg(short, long)]
    log_level: Option<String>,

    /// Save current settings as default config file (~/.mycli/config.toml)
    #[arg(long)]
    save_default: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();
    let cfg_handle = DotCfg::new("mycli");

    // 1. Load config file if it exists, or fall back to default struct
    let saved: AppConfig = cfg_handle.load()?.unwrap_or_default();

    // 2. Resolve precedence: CLI flag > Config file value > Default
    let active_config = AppConfig {
        host: args.host.unwrap_or(saved.host),
        port: args.port.unwrap_or(saved.port),
        log_level: args.log_level.unwrap_or(saved.log_level),
    };

    println!("Active configuration: {active_config:#?}");
    println!("Config file location: {:?}", cfg_handle.file_path()?);

    // Save active configuration to disk if requested
    if args.save_default {
        cfg_handle.save(&active_config)?;
        println!(
            "Saved active configuration to {:?}",
            cfg_handle.file_path()?
        );
    }

    Ok(())
}
