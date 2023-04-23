pub mod cli;
mod config;
mod utils;

use cli::KeyValType;
pub use config::{
    get_body_text, get_header_text, get_status_text, DiffConfig, DiffProfile, LoadConfig,
    RequestConfig, RequestProfile, ResponseProfile, ValidateConfig,
};
pub use utils::{diff_text, highlight_text, process_error_output};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExtraArgs {
    pub headers: Vec<(String, String)>,
    pub query: Vec<(String, String)>,
    pub body: Vec<(String, String)>,
}

impl IntoIterator for ExtraArgs {
    type Item = (KeyValType, Vec<(String, String)>);
    type IntoIter = std::array::IntoIter<(KeyValType, Vec<(String, String)>), 3>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIterator::into_iter([
            (KeyValType::Header, self.headers),
            (KeyValType::Query, self.query),
            (KeyValType::Body, self.body),
        ])
    }
}
