use anyhow::{Ok, Result};
use clap::{Parser, Subcommand};
use dialoguer::{theme::ColorfulTheme, Input};
use std::fmt::Write as _;
use std::io::Write as _;
use xdiff::{
    cli::{parse_key_value, KeyVal},
    get_body_text, get_header_text, get_status_text, highlight_text, process_error_output,
    LoadConfig, RequestConfig, RequestProfile,
};

/// Diff two http requests and compare the difference of the responses
#[derive(Parser, Debug, Clone)]
#[clap(version = "0.1.0", author = "Misky <fengwei5@foxmail.com>")]
pub struct Args {
    #[clap(subcommand)]
    action: Action,
}

#[derive(Subcommand, Debug, Clone)]
#[non_exhaustive]
enum Action {
    /// Diff two API response based on given profile.
    Run(RunArgs),

    /// Parse URLs to generate a profile.
    Parse,
}

#[derive(Parser, Debug, Clone)]
struct RunArgs {
    /// The profile name
    #[clap(short, long, value_parser)]
    profile: String,

    /// Overrides args. Could be used to override the query, headers and body of the request.
    /// for query params, use `-e key=value`
    /// for headers, use `-e %key=value`
    /// for body, use `-e @key=value`
    #[clap(short, long, value_parser = parse_key_value, number_of_values = 1)]
    extra_params: Vec<KeyVal>,

    /// Configuration to use
    #[clap(short, long, value_parser)]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let result = match args.action {
        Action::Run(args) => run(args).await,
        Action::Parse => parse().await,
        // _ => panic!("Not implemented yet"),
    };

    process_error_output(result)
}

async fn run(args: RunArgs) -> Result<()> {
    let config_file = args.config.unwrap_or_else(|| "./xdiff.yaml".to_string());
    // let profile = config
    let config = RequestConfig::load_yaml(&config_file).await?;
    let profile = config.get_profile(&args.profile).ok_or_else(|| {
        anyhow::anyhow!(
            "Profile {} not found in config file {}",
            args.profile,
            config_file
        )
    })?;

    let extra_args = args.extra_params.into();

    let url = profile.get_url(&extra_args)?;

    let res = profile.send(&extra_args).await?;
    let res = res.into_inner();

    let status = get_status_text(&res)?;
    let headers = get_header_text(&res, &[])?;
    let body = get_body_text(res, &[]).await?;

    let mut output = String::new();

    if atty::is(atty::Stream::Stdout) {
        writeln!(&mut output, "Url: {}\n", url)?;

        write!(&mut output, "{}", status)?;
        write!(&mut output, "{}", highlight_text(&headers, "yaml", None)?)?;
        write!(
            &mut output,
            "{}",
            highlight_text(&body, "json", Some("base16-mocha.dark"))?
        )?;
    } else {
        // write!(&mut output, "{}", status)?;
        // write!(&mut output, "{}", &headers)?;
        write!(&mut output, "{}", &body)?;
    }

    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    write!(stdout, "{}", output)?;

    Ok(())
}

async fn parse() -> Result<()> {
    let color_theme = ColorfulTheme::default();

    let url: String = Input::with_theme(&color_theme)
        .with_prompt("URL")
        .interact_text()?;
    let profile: RequestProfile = url.parse()?;

    let name: String = Input::with_theme(&color_theme)
        .with_prompt("Profile name")
        .interact_text()?;

    let config = RequestConfig::new(vec![(name, profile)].into_iter().collect());
    let result = serde_yaml::to_string(&config)?;

    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();

    if atty::is(atty::Stream::Stdout) {
        write!(stdout, "---\n{}", highlight_text(&result, "yaml", None)?)?;
    } else {
        write!(stdout, "{}", result)?;
    }

    Ok(())
}
