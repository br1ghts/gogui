use crossterm::event::{KeyCode, KeyEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Add,
    Edit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorField {
    Title,
    Notes,
    To,
    Subject,
    Body,
}

#[derive(Debug, Clone)]
pub enum ModalState {
    TaskEditor {
        mode: EditorMode,
        title: String,
        notes: String,
        field: EditorField,
        quick: bool,
    },
    Compose {
        to: String,
        subject: String,
        body: String,
        field: EditorField,
        is_reply: bool,
    },
    CalendarEditor {
        title: String,
        start: String,
        end: String,
        all_day: bool,
        location: String,
        description: String,
        field: EditorField,
        is_edit: bool,
    },
    Search {
        query: String,
    },
    ConfirmDelete,
}

#[derive(Debug, Clone)]
pub enum ModalSubmit {
    AddTask { title: String, notes: String },
    EditTask { title: String, notes: String },
    Search(String),
    DeleteConfirmed,
    SendEmail { to: String, subject: String, body: String },
    CalendarSave {
        title: String,
        start: String,
        end: String,
        all_day: bool,
        location: String,
        description: String,
        is_edit: bool,
    },
}

impl ModalState {
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<ModalSubmit> {
        match self {
            ModalState::TaskEditor {
                mode,
                title,
                notes,
                field,
                quick,
            } => {
                match key.code {
                    KeyCode::Tab => {
                        if !*quick {
                            *field = if *field == EditorField::Title {
                                EditorField::Notes
                            } else {
                                EditorField::Title
                            };
                        }
                    }
                    KeyCode::Backspace => {
                        let s = if *quick || *field == EditorField::Title { title } else { notes };
                        s.pop();
                    }
                    KeyCode::Enter => {
                        if title.trim().is_empty() {
                            return None;
                        }
                        return Some(match mode {
                            EditorMode::Add => ModalSubmit::AddTask {
                                title: title.trim().to_string(),
                                notes: notes.trim().to_string(),
                            },
                            EditorMode::Edit => ModalSubmit::EditTask {
                                title: title.trim().to_string(),
                                notes: notes.trim().to_string(),
                            },
                        });
                    }
                    KeyCode::Char(c) => {
                        let s = if *quick || *field == EditorField::Title { title } else { notes };
                        s.push(c);
                    }
                    _ => {}
                }
                None
            }
            ModalState::Compose {
                to,
                subject,
                body,
                field,
                ..
            } => {
                match key.code {
                    KeyCode::Tab => {
                        *field = match field {
                            EditorField::To => EditorField::Subject,
                            EditorField::Subject => EditorField::Body,
                            _ => EditorField::To,
                        }
                    }
                    KeyCode::Backspace => {
                        let s = match field {
                            EditorField::To => to,
                            EditorField::Subject => subject,
                            _ => body,
                        };
                        s.pop();
                    }
                    KeyCode::Enter => {
                        if *field == EditorField::Body {
                            if to.trim().is_empty() || subject.trim().is_empty() {
                                return None;
                            }
                            return Some(ModalSubmit::SendEmail {
                                to: to.trim().to_string(),
                                subject: subject.trim().to_string(),
                                body: body.to_string(),
                            });
                        }
                        *field = match field {
                            EditorField::To => EditorField::Subject,
                            EditorField::Subject => EditorField::Body,
                            _ => EditorField::Body,
                        }
                    }
                    KeyCode::Char(c) => {
                        let s = match field {
                            EditorField::To => to,
                            EditorField::Subject => subject,
                            _ => body,
                        };
                        s.push(c);
                    }
                    _ => {}
                }
                None
            }
            ModalState::Search { query } => {
                match key.code {
                    KeyCode::Backspace => {
                        query.pop();
                    }
                    KeyCode::Enter => return Some(ModalSubmit::Search(query.trim().to_string())),
                    KeyCode::Char(c) => query.push(c),
                    _ => {}
                }
                None
            }
            ModalState::CalendarEditor {
                title,
                start,
                end,
                all_day,
                location,
                description,
                field,
                is_edit,
            } => {
                match key.code {
                    KeyCode::Tab => {
                        *field = match field {
                            EditorField::Title => EditorField::To,
                            EditorField::To => EditorField::Subject,
                            EditorField::Subject => EditorField::Body,
                            EditorField::Body => EditorField::Notes,
                            EditorField::Notes => EditorField::Title,
                        };
                    }
                    KeyCode::Backspace => {
                        let s = match field {
                            EditorField::Title => title,
                            EditorField::To => start,
                            EditorField::Subject => end,
                            EditorField::Body => location,
                            EditorField::Notes => description,
                        };
                        s.pop();
                    }
                    KeyCode::Enter => {
                        return Some(ModalSubmit::CalendarSave {
                            title: title.trim().to_string(),
                            start: start.trim().to_string(),
                            end: end.trim().to_string(),
                            all_day: *all_day,
                            location: location.trim().to_string(),
                            description: description.to_string(),
                            is_edit: *is_edit,
                        });
                    }
                    KeyCode::Char(' ') if *field == EditorField::Title => {
                        *all_day = !*all_day;
                    }
                    KeyCode::Char(c) => {
                        let s = match field {
                            EditorField::Title => title,
                            EditorField::To => start,
                            EditorField::Subject => end,
                            EditorField::Body => location,
                            EditorField::Notes => description,
                        };
                        s.push(c);
                    }
                    _ => {}
                }
                None
            }
            ModalState::ConfirmDelete => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => Some(ModalSubmit::DeleteConfirmed),
                _ => None,
            },
        }
    }
}
