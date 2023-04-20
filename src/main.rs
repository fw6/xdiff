use std::io::Write;

use anyhow::{Ok, Result};
use clap::Parser;
use dialoguer::{theme::ColorfulTheme, Input, MultiSelect};
use serde_yaml;
use xdiff::{
    cli::{Action, Args, RunArgs},
    highlight_text, DiffConfig, DiffProfile, ExtraArgs, RequestProfile, ResponseProfile,
};

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    // let config = args.config();

    match args.action {
        Action::Run(args) => run(args).await?,
        Action::Parse => parse().await?,
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

async fn parse() -> Result<()> {
    let color_theme = ColorfulTheme::default();

    let url1: String = Input::with_theme(&color_theme)
        .with_prompt("URL 1")
        .interact_text()?;
    let url2: String = Input::with_theme(&color_theme)
        .with_prompt("URL 2")
        .interact_text()?;
    let name: String = Input::with_theme(&color_theme)
        .with_prompt("Profile Name")
        .interact_text()?;

    let req1: RequestProfile = url1.parse()?;
    let req2: RequestProfile = url2.parse()?;

    let res1 = req1.send(&ExtraArgs::default()).await?;
    // let res2 = req2.send(&ExtraArgs::default()).await?;

    let headers = res1.get_header_keys();

    let chosen = MultiSelect::with_theme(&color_theme)
        .with_prompt("Select headers to skip")
        .items(&headers)
        .interact()?;

    let skip_headers = chosen.iter().map(|i| headers[*i].to_string()).collect();

    let res = ResponseProfile::new(skip_headers, vec![]);

    let profile = DiffProfile::new(req1, req2, res);
    let config = DiffConfig::new(vec![(name, profile)].into_iter().collect());
    let result = serde_yaml::to_string(&config)?;

    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();

    write!(stdout, "{}", highlight_text(&result, "yaml")?)?;

    Ok(())
}
