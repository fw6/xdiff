use anyhow::{anyhow, Ok, Result};
use async_trait::async_trait;
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE},
    Client, Method, Response, Url,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use serde_urlencoded;
use serde_yaml;
use std::fmt::Write;
use std::str::FromStr;
use tokio::fs;

use crate::{cli::KeyValType, ExtraArgs};

mod xdiff;
mod xreq;

pub use xdiff::{DiffConfig, DiffProfile, ResponseProfile};
pub use xreq::RequestConfig;

#[async_trait]
pub trait LoadConfig
where
    Self: Sized + DeserializeOwned + ValidateConfig,
{
    /// Load config from yaml file
    async fn load_yaml(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path).await?;
        Self::from_yaml(&content)
    }

    /// Load config from yaml string
    fn from_yaml(content: &str) -> Result<Self> {
        let config: Self = serde_yaml::from_str(content)?;
        config.validate()?;
        Ok(config)
    }
}

pub trait ValidateConfig {
    fn validate(&self) -> Result<()>;
}

pub fn is_default<T: Default + PartialEq>(t: &T) -> bool {
    t == &T::default()
}

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

    fn generate(&self, args: &ExtraArgs) -> Result<(HeaderMap, serde_json::Value, String)> {
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

    pub fn get_url(&self, args: &ExtraArgs) -> Result<String> {
        let (_, params, _) = self.generate(args)?;
        // let params = self.params.clone();
        let mut url = self.url.clone();

        if !params.as_object().unwrap().is_empty() {
            let query = serde_qs::to_string(&params)?;
            url.set_query(Some(&query));
        }
        Ok(url.into())
    }
}

impl ValidateConfig for RequestProfile {
    fn validate(&self) -> Result<()> {
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
    pub fn into_inner(self) -> Response {
        self.0
    }

    pub async fn get_text(self, profile: &ResponseProfile) -> Result<String> {
        let res = self.0;

        let mut output = get_status_text(&res)?;
        write!(
            &mut output,
            "{}",
            get_header_text(&res, &profile.skip_headers)?
        )?;

        write!(
            &mut output,
            "{}",
            get_body_text(res, &profile.skip_body).await?
        )?;

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

pub fn get_header_text(res: &Response, skip_headers: &[String]) -> Result<String> {
    let headers = res.headers();
    let mut output = String::new();

    for (k, v) in headers.iter() {
        if !skip_headers.contains(&k.to_string()) {
            writeln!(output, "{}:{:?}", k, v)?;
        }
    }

    writeln!(output)?;

    Ok(output)
}

pub fn get_status_text(res: &Response) -> Result<String> {
    Ok(format!("{:?} {}", res.version(), res.status()))
}

pub async fn get_body_text(res: Response, skip_body: &[String]) -> Result<String> {
    let content_type = get_content_type(&res.headers());
    let text = res.text().await?;

    match content_type.as_deref() {
        Some("application/json") => filter_json(&text, skip_body),
        _ => Ok(text),
    }
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

#[cfg(test)]
mod tests {
    use reqwest::{header, StatusCode};

    use super::*;

    #[tokio::test]
    async fn request_profile_send_should_work() {
        let mut server = mockito::Server::new();

        let _mock = mock_for_url(
            &mut server,
            "/todo?a=1&b=2",
            json!({"id":1, "title": "todo"}),
        );

        let res = get_response(server, "/todo?a=1&b=2", &Default::default()).await;

        assert_eq!(res.into_inner().status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn request_profile_send_with_args_should_work() {
        let mut server = mockito::Server::new();

        let _mock = mock_for_url(
            &mut server,
            "/todo?a=1&b=2",
            json!({"id":1, "title": "todo"}),
        );
        let args =
            ExtraArgs::new_with_query(vec![("a".into(), "1".into()), ("b".into(), "2".into())]);

        let res = get_response(server, "/todo?a=1&b=2", &args).await;

        assert_eq!(res.into_inner().status(), StatusCode::OK);
    }

    #[test]
    fn request_profile_get_url_should_work() {
        let profile = get_profile("http://localhost:8080", "/todo?c=3&d=4");

        assert_eq!(
            profile.get_url(&Default::default()).unwrap(),
            "http://localhost:8080/todo?c=3&d=4"
        );
    }

    #[test]
    fn request_profile_get_url_with_args_should_work() {
        let profile = get_profile("http://localhost:8080", "/todo?a=1&b=2");

        let args =
            ExtraArgs::new_with_query(vec![("b".into(), "2".into()), ("a".into(), "1".into())]);

        assert_eq!(
            profile.get_url(&args).unwrap(),
            "http://localhost:8080/todo?a=1&b=2"
        );
    }

    #[test]
    fn test_get_content_type() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json; charset=utf-8"),
        );

        assert_eq!(
            get_content_type(&headers),
            Some("application/json".to_string())
        );
    }

    #[tokio::test]
    async fn get_status_text_should_work() {
        let mut server_guard = mockito::Server::new();
        let _m = mock_for_url(
            &mut server_guard,
            "/todo",
            json!({"id": 1, "title": "todo"}),
        );
        let res = get_response(server_guard, "/todo", &Default::default()).await;

        assert_eq!(
            get_status_text(&res.into_inner()).unwrap(),
            "HTTP/1.1 200 OK"
        );
    }

    #[tokio::test]
    async fn get_header_text_should_work() {
        let mut server_guard = mockito::Server::new();
        let _m = mock_for_url(
            &mut server_guard,
            "/todo",
            json!({"id": 1, "title": "todo"}),
        );
        let res = get_response(server_guard, "/todo", &Default::default()).await;

        assert_eq!(
            get_header_text(
                &res.into_inner(),
                &["connection".into(), "content-length".into(), "date".into()]
            )
            .unwrap(),
            "content-type:\"application/json\"\n\n"
        );
    }

    #[tokio::test]
    async fn get_body_text_should_work() {
        let mut server_guard = mockito::Server::new();
        let body = json!({"id": 1, "title": "todo"});
        let _m = mock_for_url(&mut server_guard, "/todo", body);
        let res = get_response(server_guard, "/todo", &Default::default()).await;

        assert_eq!(
            get_body_text(res.into_inner(), &[]).await.unwrap(),
            serde_json::to_string_pretty(&json!({"id": 1, "title": "todo"})).unwrap()
        );
    }

    fn mock_for_url(
        server_guard: &mut mockito::ServerGuard,
        path_and_query: &str,
        res_body: serde_json::Value,
    ) -> mockito::Mock {
        server_guard
            .mock("GET", path_and_query)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&res_body).unwrap())
            .create()
    }

    async fn get_response(
        server: mockito::ServerGuard,
        path_and_query: &str,
        args: &ExtraArgs,
    ) -> ResponseExt {
        let profile = get_profile(&server.url(), &path_and_query);

        profile.send(&args).await.unwrap()
    }

    fn get_profile(url: &str, path_and_query: &str) -> RequestProfile {
        let url = get_url(url, path_and_query);
        // RequestProfile::new(Method::GET, url, params, HeaderMap::new(), None)

        RequestProfile::from_str(url.as_str()).unwrap()
    }

    fn get_url(url: &str, path: &str) -> Url {
        Url::parse(&format!("{}{}", url, path)).unwrap()
    }
}
