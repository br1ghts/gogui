use crate::gmail::models::{GmailLabel, MessageDetail, ThreadHeader};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GmailPane {
    Labels,
    Threads,
    Detail,
}

impl Default for GmailPane {
    fn default() -> Self {
        Self::Labels
    }
}

#[derive(Debug, Default, Clone)]
pub struct GmailState {
    pub labels: Vec<GmailLabel>,
    pub threads: Vec<ThreadHeader>,
    pub selected_label: usize,
    pub selected_thread: usize,
    pub active_pane: GmailPane,
    pub search_query: String,
    pub detail: Option<MessageDetail>,
}

impl GmailState {
    pub fn selected_label_id(&self) -> Option<&str> {
        self.labels.get(self.selected_label).map(|l| l.id.as_str())
    }

    pub fn selected_label_name(&self) -> String {
        self.labels
            .get(self.selected_label)
            .map(|l| l.name.clone())
            .unwrap_or_else(|| "INBOX".to_string())
    }

    pub fn selected_thread_id(&self) -> Option<&str> {
        self.threads.get(self.selected_thread).map(|t| t.id.as_str())
    }

    pub fn selected_thread(&self) -> Option<&ThreadHeader> {
        self.threads.get(self.selected_thread)
    }

    pub fn set_labels(&mut self, mut labels: Vec<GmailLabel>) {
        labels.sort_by(|a, b| a.name.cmp(&b.name));
        self.labels = labels;
        if let Some(pos) = self.labels.iter().position(|l| l.name.eq_ignore_ascii_case("INBOX")) {
            self.selected_label = pos;
        } else if self.selected_label >= self.labels.len() {
            self.selected_label = self.labels.len().saturating_sub(1);
        }
    }

    pub fn set_threads(&mut self, threads: Vec<ThreadHeader>) {
        self.threads = threads;
        if self.selected_thread >= self.threads.len() {
            self.selected_thread = self.threads.len().saturating_sub(1);
        }
    }

    pub fn pane_next(&mut self) {
        self.active_pane = match self.active_pane {
            GmailPane::Labels => GmailPane::Threads,
            GmailPane::Threads => GmailPane::Detail,
            GmailPane::Detail => GmailPane::Labels,
        };
    }

    pub fn move_up(&mut self) {
        match self.active_pane {
            GmailPane::Labels => {
                if self.selected_label > 0 {
                    self.selected_label -= 1;
                    self.selected_thread = 0;
                }
            }
            GmailPane::Threads => {
                if self.selected_thread > 0 {
                    self.selected_thread -= 1;
                }
            }
            GmailPane::Detail => {}
        }
    }

    pub fn move_down(&mut self) {
        match self.active_pane {
            GmailPane::Labels => {
                if self.selected_label + 1 < self.labels.len() {
                    self.selected_label += 1;
                    self.selected_thread = 0;
                }
            }
            GmailPane::Threads => {
                if self.selected_thread + 1 < self.threads.len() {
                    self.selected_thread += 1;
                }
            }
            GmailPane::Detail => {}
        }
    }
}
