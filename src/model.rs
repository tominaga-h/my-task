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
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Open,
    Done,
}

impl Status {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &str {
        match self {
            Status::Open => "open",
            Status::Done => "done",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "done" => Status::Done,
            _ => Status::Open,
        }
    }
}
