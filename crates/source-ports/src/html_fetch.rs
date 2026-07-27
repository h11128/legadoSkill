//! HtmlFetchPort — PC HTML fetch (infra implements with reqwest later).

use source_types::{FetchResult, HeaderMap, PortError, Url};

pub trait HtmlFetchPort {
    fn fetch(&self, url: &Url, headers: &HeaderMap) -> Result<FetchResult, PortError>;
}
