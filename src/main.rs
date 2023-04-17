use std::io::Write;

use anyhow::{Ok, Result};
use clap::Parser;
use xdiff::{
    cli::{Action, Args, RunArgs},
    DiffConfig, ExtraArgs,
};

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    // let config = args.config();

    match args.action {
        Action::Run(args) => run(args).await?,
        _ => panic!("Not implemented yet"),
    }

    Ok(())
}

async fn run(args: RunArgs) -> Result<()> {
    let config_file = args.config.unwrap_or_else(|| "./xdiff.yaml".to_string());
    // let profile = config
    let config = DiffConfig::load_yaml(&config_file).await?;
    let profile = config.get_profile(&args.profile).ok_or_else(|| {
        anyhow::anyhow!(
            "Profile {} not found in config file {}",
            args.profile,
            config_file
        )
    })?;

    let extra_args = ExtraArgs::from(args.extra_params);
    let result = profile.diff(extra_args).await?;

    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    write!(stdout, "{}", result)?;

    Ok(())
}
