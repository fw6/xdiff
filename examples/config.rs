use anyhow::Result;
// use xdiff::DiffConfig;
use xdiff::{LoadConfig, RequestConfig};

fn main() -> Result<()> {
    let content = include_str!("../fixtures/xreq_test.yaml");
    let config = RequestConfig::from_yaml(content)?;

    println!("{:#?}", config);
    Ok(())
}
