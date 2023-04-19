pub mod cli;
mod config;
mod req;
mod utils;

use cli::KeyValType;
pub use config::{DiffConfig, DiffProfile, ResponseProfile};
pub use req::RequestProfile;
pub use utils::diff_text;

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
