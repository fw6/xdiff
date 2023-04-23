use std::collections::HashMap;

use crate::diff_text;
use crate::ExtraArgs;
use anyhow::{Context, Result};
use serde::Deserialize;
use serde::Serialize;

use super::LoadConfig;
use super::ValidateConfig;
use super::{is_default, RequestProfile};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DiffConfig {
    #[serde(flatten)]
    pub profiles: HashMap<String, DiffProfile>,
}

impl LoadConfig for DiffConfig {}
impl ValidateConfig for DiffConfig {
    fn validate(&self) -> Result<()> {
        for (name, profile) in &self.profiles {
            profile
                .validate()
                .context(format!("failed to validate profile: {}", name))?;
        }
        Ok(())
    }
}

impl DiffConfig {
    pub fn new(profiles: HashMap<String, DiffProfile>) -> Self {
        Self { profiles }
    }

    pub fn get_profile(&self, name: &str) -> Option<&DiffProfile> {
        self.profiles.get(name)
    }
}

impl DiffProfile {
    pub async fn diff(&self, args: ExtraArgs) -> Result<String> {
        let res1 = self.req1.send(&args).await?;
        let res2 = self.req2.send(&args).await?;

        let text1 = res1.get_text(&self.res).await?;
        let text2 = res2.get_text(&self.res).await?;

        diff_text(&text1, &text2)
    }
}

impl ValidateConfig for DiffProfile {
    fn validate(&self) -> Result<()> {
        self.req1.validate().context("req1 failed to validate")?;
        self.req2.validate().context("req2 failed to validate")?;

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiffProfile {
    pub req1: RequestProfile,
    pub req2: RequestProfile,

    #[serde(skip_serializing_if = "is_default", default)]
    pub res: ResponseProfile,
}

impl DiffProfile {
    pub fn new(req1: RequestProfile, req2: RequestProfile, res: ResponseProfile) -> Self {
        Self { req1, req2, res }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub struct ResponseProfile {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub skip_headers: Vec<String>,

    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub skip_body: Vec<String>,
}

impl ResponseProfile {
    pub fn new(skip_headers: Vec<String>, skip_body: Vec<String>) -> Self {
        Self {
            skip_headers,
            skip_body,
        }
    }
}
