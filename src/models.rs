use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskList {
    pub id: String,
    pub title: String,
    pub updated: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub notes: Option<String>,
    pub due: Option<String>,
    pub status: Option<String>,
    pub updated: Option<String>,
    pub completed: Option<String>,
    pub deleted: Option<bool>,
    pub hidden: Option<bool>,
}

impl Task {
    pub fn is_completed(&self) -> bool {
        self.status.as_deref() == Some("completed")
    }
}

#[derive(Debug, Deserialize)]
pub struct TaskListsResponse {
    pub items: Option<Vec<TaskList>>,
}

#[derive(Debug, Deserialize)]
pub struct TasksResponse {
    pub items: Option<Vec<Task>>,
}

#[derive(Debug, Serialize)]
pub struct InsertTaskRequest<'a> {
    pub title: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<&'a str>,
}

#[derive(Debug, Serialize)]
pub struct PatchTaskRequest<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<&'a str>,
}
