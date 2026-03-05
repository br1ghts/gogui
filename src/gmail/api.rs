use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct LabelsResponse {
    pub labels: Option<Vec<LabelDto>>,
}

#[derive(Debug, Deserialize)]
pub struct LabelDto {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub label_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ThreadsListResponse {
    pub threads: Option<Vec<ThreadIdDto>>,
}

#[derive(Debug, Deserialize)]
pub struct ThreadIdDto {
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct ThreadResponse {
    pub messages: Option<Vec<MessageResponse>>,
}

#[derive(Debug, Deserialize)]
pub struct MessageResponse {
    pub id: String,
    #[serde(rename = "threadId")]
    pub thread_id: Option<String>,
    #[serde(rename = "labelIds")]
    pub label_ids: Option<Vec<String>>,
    pub snippet: Option<String>,
    pub payload: Option<MessagePart>,
}

#[derive(Debug, Deserialize)]
pub struct MessagePart {
    pub headers: Option<Vec<Header>>, 
    pub body: Option<MessageBody>,
    pub parts: Option<Vec<MessagePart>>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MessageBody {
    pub data: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Header {
    pub name: String,
    pub value: String,
}
