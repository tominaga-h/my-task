use chrono::NaiveDate;

#[derive(Debug, Clone)]
pub struct Task {
    pub id: u32,
    pub title: String,
    pub status: Status,
    #[allow(dead_code)]
    pub source: String,
    pub created: NaiveDate,
    pub project: Option<String>,
    pub due: Option<NaiveDate>,
    pub done_at: Option<NaiveDate>,
    #[allow(dead_code)]
    pub updated: NaiveDate,
    pub reminds: Vec<NaiveDate>,
    pub important: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Open,
    Done,
    Closed,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum SortKey {
    #[default]
    Id,
    Due,
    Project,
    Created,
}

impl SortKey {
    pub fn as_sql(&self) -> &str {
        match self {
            SortKey::Id => "id",
            SortKey::Due => "due IS NULL, due",
            SortKey::Project => "project IS NULL, project",
            SortKey::Created => "created",
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub enum SortOrder {
    #[default]
    Asc,
    Desc,
}

impl SortOrder {
    pub fn as_sql(&self) -> &str {
        match self {
            SortOrder::Asc => "ASC",
            SortOrder::Desc => "DESC",
        }
    }
}

impl Status {
    pub fn as_str(&self) -> &str {
        match self {
            Status::Open => "open",
            Status::Done => "done",
            Status::Closed => "closed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "done" => Status::Done,
            "closed" => Status::Closed,
            _ => Status::Open,
        }
    }
}
