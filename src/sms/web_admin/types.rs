use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct ListQuery {
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<usize>,
    pub(crate) offset: Option<usize>,
    pub(crate) sort: Option<String>,
    pub(crate) sort_by: Option<String>,
    pub(crate) order: Option<String>,
    pub(crate) q: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct PageTokenQuery {
    pub(crate) limit: Option<i32>,
    pub(crate) page_token: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct StreamQuery {
    pub(crate) once: Option<bool>,
}
