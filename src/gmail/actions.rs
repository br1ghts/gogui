use crate::gmail::client::{GmailClient, GmailError};

pub async fn archive_thread(client: &mut GmailClient, thread_id: &str) -> Result<(), GmailError> {
    client.modify_thread_labels(thread_id, &[], &["INBOX"]).await
}

pub async fn toggle_unread(client: &mut GmailClient, thread_id: &str, unread_now: bool) -> Result<(), GmailError> {
    if unread_now {
        client.modify_thread_labels(thread_id, &["UNREAD"], &[]).await
    } else {
        client.modify_thread_labels(thread_id, &[], &["UNREAD"]).await
    }
}

pub fn build_raw_email(to: &str, subject: &str, body: &str, in_reply_to: Option<&str>) -> String {
    let mut out = String::new();
    out.push_str(&format!("To: {}\r\n", to.trim()));
    out.push_str(&format!("Subject: {}\r\n", subject.trim()));
    out.push_str("MIME-Version: 1.0\r\n");
    out.push_str("Content-Type: text/plain; charset=UTF-8\r\n");
    if let Some(id) = in_reply_to {
        out.push_str(&format!("In-Reply-To: {}\r\n", id));
        out.push_str(&format!("References: {}\r\n", id));
    }
    out.push_str("\r\n");
    out.push_str(body);
    out
}
