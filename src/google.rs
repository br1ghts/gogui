use reqwest::Client;

use crate::api_error::{map_http_error, ApiError};
use crate::models::{InsertTaskRequest, PatchTaskRequest, Task, TaskList, TaskListsResponse, TasksResponse};

pub struct GoogleTasksClient {
    http: Client,
}

impl GoogleTasksClient {
    pub fn new() -> Self {
        Self {
            http: Client::new(),
        }
    }

    pub async fn list_tasklists(&self, access_token: &str) -> Result<Vec<TaskList>, ApiError> {
        let res = self
            .http
            .get("https://tasks.googleapis.com/tasks/v1/users/@me/lists")
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| ApiError::Other(format!("Tasklists request failed: {e}")))?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(map_http_error(status, &body));
        }

        let payload: TaskListsResponse = res
            .json()
            .await
            .map_err(|e| ApiError::Other(format!("Tasklists JSON decode failed: {e}")))?;
        Ok(payload.items.unwrap_or_default())
    }

    pub async fn list_tasks(
        &self,
        access_token: &str,
        tasklist_id: &str,
        show_completed: bool,
    ) -> Result<Vec<Task>, ApiError> {
        let res = self
            .http
            .get(format!(
                "https://tasks.googleapis.com/tasks/v1/lists/{tasklist_id}/tasks"
            ))
            .query(&[
                ("showCompleted", if show_completed { "true" } else { "false" }),
                ("showHidden", "false"),
                ("showDeleted", "false"),
            ])
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| ApiError::Other(format!("Tasks request failed: {e}")))?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(map_http_error(status, &body));
        }

        let payload: TasksResponse = res
            .json()
            .await
            .map_err(|e| ApiError::Other(format!("Tasks JSON decode failed: {e}")))?;
        Ok(payload.items.unwrap_or_default())
    }

    pub async fn add_task(
        &self,
        access_token: &str,
        tasklist_id: &str,
        title: &str,
        notes: Option<&str>,
    ) -> Result<(), ApiError> {
        let body = InsertTaskRequest { title, notes };
        let res = self
            .http
            .post(format!(
                "https://tasks.googleapis.com/tasks/v1/lists/{tasklist_id}/tasks"
            ))
            .bearer_auth(access_token)
            .json(&body)
            .send()
            .await
            .map_err(|e| ApiError::Other(format!("Add task request failed: {e}")))?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(map_http_error(status, &body));
        }

        Ok(())
    }

    pub async fn edit_task(
        &self,
        access_token: &str,
        tasklist_id: &str,
        task_id: &str,
        title: &str,
        notes: Option<&str>,
    ) -> Result<(), ApiError> {
        let body = PatchTaskRequest {
            title: Some(title),
            notes,
            status: None,
            completed: None,
        };
        let res = self
            .http
            .patch(format!(
                "https://tasks.googleapis.com/tasks/v1/lists/{tasklist_id}/tasks/{task_id}"
            ))
            .bearer_auth(access_token)
            .json(&body)
            .send()
            .await
            .map_err(|e| ApiError::Other(format!("Edit task request failed: {e}")))?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(map_http_error(status, &body));
        }

        Ok(())
    }

    pub async fn toggle_complete(
        &self,
        access_token: &str,
        tasklist_id: &str,
        task_id: &str,
        complete: bool,
    ) -> Result<(), ApiError> {
        let body = if complete {
            PatchTaskRequest {
                title: None,
                notes: None,
                status: Some("completed"),
                completed: Some("now"),
            }
        } else {
            PatchTaskRequest {
                title: None,
                notes: None,
                status: Some("needsAction"),
                completed: None,
            }
        };

        let res = self
            .http
            .patch(format!(
                "https://tasks.googleapis.com/tasks/v1/lists/{tasklist_id}/tasks/{task_id}"
            ))
            .bearer_auth(access_token)
            .json(&body)
            .send()
            .await
            .map_err(|e| ApiError::Other(format!("Toggle complete request failed: {e}")))?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(map_http_error(status, &body));
        }

        Ok(())
    }

    pub async fn delete_task(
        &self,
        access_token: &str,
        tasklist_id: &str,
        task_id: &str,
    ) -> Result<(), ApiError> {
        let res = self
            .http
            .delete(format!(
                "https://tasks.googleapis.com/tasks/v1/lists/{tasklist_id}/tasks/{task_id}"
            ))
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| ApiError::Other(format!("Delete task request failed: {e}")))?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(map_http_error(status, &body));
        }

        Ok(())
    }
}
