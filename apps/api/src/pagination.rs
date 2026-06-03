use serde::{Deserialize, Serialize};

use crate::errors::ApiError;

const DEFAULT_PAGE: i64 = 1;
const DEFAULT_PAGE_SIZE: i64 = 20;
const MAX_PAGE_SIZE: i64 = 100;

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug)]
pub struct Pagination {
    pub page: i64,
    pub page_size: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Serialize)]
pub struct Paginated<T> {
    pub items: Vec<T>,
    pub page: i64,
    pub page_size: i64,
    pub total: i64,
}

impl Pagination {
    pub fn from_query(query: PaginationQuery) -> Result<Self, ApiError> {
        let page = query.page.unwrap_or(DEFAULT_PAGE);
        let page_size = query.page_size.unwrap_or(DEFAULT_PAGE_SIZE);

        if page < 1 {
            return Err(ApiError::bad_request("page doit etre superieur ou egal a 1"));
        }

        if !(1..=MAX_PAGE_SIZE).contains(&page_size) {
            return Err(ApiError::bad_request(format!(
                "page_size doit etre entre 1 et {MAX_PAGE_SIZE}"
            )));
        }

        Ok(Self {
            page,
            page_size,
            limit: page_size,
            offset: (page - 1) * page_size,
        })
    }
}

impl<T> Paginated<T> {
    pub fn new(items: Vec<T>, pagination: &Pagination, total: i64) -> Self {
        Self {
            items,
            page: pagination.page,
            page_size: pagination.page_size,
            total,
        }
    }
}
