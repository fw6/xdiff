use crate::ExtraArgs;
use anyhow::{anyhow, Ok, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyVal {
    pub key_type: KeyValType,
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyValType {
    Query,
    Header,
    Body,
}

pub fn parse_key_value(s: &str) -> Result<KeyVal> {
    let mut parts = s.splitn(2, '=');

    let key = parts
        .next()
        .ok_or_else(|| anyhow!("Invalid key value pair"))?
        .trim();
    let value = parts
        .next()
        .ok_or_else(|| anyhow!("Invalid key value pair"))?
        .trim();

    let (key_type, key) = match key.chars().next() {
        Some('%') => (KeyValType::Header, &key[1..]),
        Some('@') => (KeyValType::Body, &key[1..]),
        Some(v) if v.is_alphabetic() => (KeyValType::Query, key),
        _ => return Err(anyhow!("Invalid key value pair")),
    };

    let key = match key_type {
        KeyValType::Header => key.to_string(),
        KeyValType::Body => key.to_string(),
        _ => key.to_string(),
    };

    Ok(KeyVal {
        key_type,
        key: key.to_string(),
        value: value.to_string(),
    })
}

impl From<Vec<KeyVal>> for ExtraArgs {
    fn from(args: Vec<KeyVal>) -> Self {
        let mut headers = vec![];
        let mut query = vec![];
        let mut body = vec![];

        for arg in args {
            match arg.key_type {
                KeyValType::Header => headers.push((arg.key, arg.value)),
                KeyValType::Query => query.push((arg.key, arg.value)),
                KeyValType::Body => body.push((arg.key, arg.value)),
            }
        }

        Self {
            headers,
            query,
            body,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn from_vec_key_val_for_extra_args_should_work() {
        let args = vec![
            KeyVal {
                key_type: KeyValType::Header,
                key: "Content-Type".to_string(),
                value: "application/json".to_string(),
            },
            KeyVal {
                key_type: KeyValType::Query,
                key: "id".to_string(),
                value: "1".to_string(),
            },
            KeyVal {
                key_type: KeyValType::Body,
                key: "name".to_string(),
                value: "misky".to_string(),
            },
        ];

        let extra_args = ExtraArgs::from(args);

        assert_eq!(
            extra_args,
            ExtraArgs {
                headers: vec![("Content-Type".to_string(), "application/json".to_string())],
                query: vec![("id".to_string(), "1".to_string())],
                body: vec![("name".to_string(), "misky".to_string())],
            }
        )
    }

    #[test]
    fn parse_key_val_should_work() {
        let args = vec!["%Content-Type=application/json", "id=1", "@name=misky"];

        let key_vals = args
            .into_iter()
            .map(|s| parse_key_value(s))
            .collect::<Result<Vec<_>>>()
            .unwrap();

        assert_eq!(
            key_vals,
            vec![
                KeyVal {
                    key_type: KeyValType::Header,
                    key: "Content-Type".to_string(),
                    value: "application/json".to_string(),
                },
                KeyVal {
                    key_type: KeyValType::Query,
                    key: "id".to_string(),
                    value: "1".to_string(),
                },
                KeyVal {
                    key_type: KeyValType::Body,
                    key: "name".to_string(),
                    value: "misky".to_string(),
                },
            ]
        )
    }
}
