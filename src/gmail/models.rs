use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GmailLabel {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub label_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadHeader {
    pub id: String,
    pub last_message_id: Option<String>,
    pub from: String,
    pub subject: String,
    pub date: String,
    pub unread: bool,
    pub snippet: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MessageDetail {
    pub id: String,
    pub thread_id: String,
    pub from: String,
    pub to: String,
    pub subject: String,
    pub date: String,
    pub snippet: String,
    pub labels: Vec<String>,
    pub body: String,
}
