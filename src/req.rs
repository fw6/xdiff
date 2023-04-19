use anyhow::{anyhow, Ok, Result};
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE},
    Client, Method, Response, Url,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_urlencoded;
use std::fmt::Write;
use std::str::FromStr;

use crate::{cli::KeyValType, ExtraArgs, ResponseProfile};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RequestProfile {
    #[serde(with = "http_serde::method", default)]
    pub method: Method,

    pub url: Url,

    #[serde(skip_serializing_if = "empty_json_value", default)]
    pub params: Option<serde_json::Value>,

    #[serde(
        skip_serializing_if = "HeaderMap::is_empty",
        with = "http_serde::header_map",
        default
    )]
    pub headers: HeaderMap,

    #[serde(skip_serializing_if = "empty_json_value", default)]
    pub body: Option<serde_json::Value>,
}

pub struct ResponseExt(Response);

impl RequestProfile {
    pub async fn send(&self, args: &ExtraArgs) -> Result<ResponseExt> {
        let (headers, query, body) = self.generate(&args)?;

        let client = Client::new();
        let req = client
            .request(self.method.clone(), self.url.clone())
            .query(&query)
            .headers(headers)
            .body(body)
            .build()?;

        let res = client.execute(req).await?;

        Ok(ResponseExt(res))
    }

    pub(crate) fn validate(&self) -> Result<()> {
        if let Some(params) = self.params.as_ref() {
            if !params.is_object() {
                return Err(anyhow!(
                    "params must be an object but got\n{}",
                    serde_yaml::to_string(params)?
                ));
            }
        }

        if let Some(body) = self.body.as_ref() {
            if !body.is_object() {
                return Err(anyhow!(
                    "body must be an object but got\n{}",
                    serde_yaml::to_string(body)?
                ));
            }
        }

        Ok(())
    }

    pub fn generate(&self, args: &ExtraArgs) -> Result<(HeaderMap, serde_json::Value, String)> {
        let mut header = self.headers.clone();
        let mut query = self.params.clone().unwrap_or_else(|| json!({}));
        let mut body = self.body.clone().unwrap_or_else(|| json!({}));

        for (key_value_type, value) in args.clone().into_iter() {
            match key_value_type {
                KeyValType::Header => {
                    for (key, value) in &value {
                        header.insert(HeaderName::from_str(key)?, HeaderValue::from_str(value)?);
                    }

                    if !header.contains_key(CONTENT_TYPE) {
                        header.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
                    }
                }
                KeyValType::Query => {
                    for (key, value) in &value {
                        query[key] = value.parse()?;
                    }
                }
                KeyValType::Body => {
                    for (key, value) in &value {
                        body[key] = value.parse()?;
                    }
                }
            }
        }

        let content_type = get_content_type(&header);

        match content_type.as_deref() {
            Some("application/json") => {
                let body = serde_json::to_string(&body)?;
                Ok((header, query, body))
            }
            Some("application/x-www-form-urlencoded" | "multipart/form-data") => {
                let body = serde_urlencoded::to_string(&body)?;
                Ok((header, query, body))
            }
            _ => Err(anyhow::anyhow!("Unsupported content type!")),
        }
    }

    pub fn new(
        method: Method,
        url: Url,
        params: Option<serde_json::Value>,
        headers: HeaderMap,
        body: Option<serde_json::Value>,
    ) -> Self {
        Self {
            method,
            url,
            params,
            headers,
            body,
        }
    }
}

impl FromStr for RequestProfile {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let mut url = Url::parse(s)?;

        let qs = url.query_pairs();
        let mut params = json!({});
        for (k, v) in qs {
            params[&*k] = v.parse()?;
        }

        url.set_query(None);

        Ok(RequestProfile::new(
            Method::GET,
            url,
            Some(params),
            HeaderMap::new(),
            None,
        ))
    }
}

impl ResponseExt {
    pub async fn get_text(self, profile: &ResponseProfile) -> Result<String> {
        let res = self.0;

        let mut output = get_header_text(&res, &profile.skip_headers)?;

        let content_type = get_content_type(res.headers());
        let text = res.text().await?;

        match content_type.as_deref() {
            Some("application/json") => {
                let text = filter_json(&text, &profile.skip_body)?;
                writeln!(output, "{}", &text)?;
            }
            _ => {
                writeln!(output, "{}", &text)?;
            }
        }

        Ok(output)
    }

    pub fn get_header_keys(&self) -> Vec<String> {
        self.0
            .headers()
            .keys()
            .map(|k| k.as_str().to_string())
            .collect()
    }
}

pub fn get_content_type(headers: &HeaderMap) -> Option<String> {
    headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().unwrap().split(';').next().map(|s| s.to_string()))
}

fn filter_json(text: &str, skip_body: &[String]) -> Result<String> {
    let mut json = serde_json::from_str::<serde_json::Value>(text)?;

    if let serde_json::Value::Object(ref mut obj) = json {
        for k in skip_body {
            obj.remove(k);
        }
    }

    Ok(serde_json::to_string_pretty(&json)?)
}

fn get_header_text(res: &Response, skip_headers: &[String]) -> Result<String> {
    let headers = res.headers();
    let mut output = String::new();

    writeln!(output, "{:?} {}", res.version(), res.status())?;

    for (k, v) in headers.iter() {
        if !skip_headers.contains(&k.to_string()) {
            writeln!(output, "{}:{:?}", k, v)?;
        }
    }

    writeln!(output)?;

    Ok(output)
}

fn empty_json_value(v: &Option<serde_json::Value>) -> bool {
    v.as_ref().map_or(true, |v| {
        if v.is_object() {
            if let Some(obj) = v.as_object() {
                return obj.is_empty();
            }
        }

        true
    })
}
