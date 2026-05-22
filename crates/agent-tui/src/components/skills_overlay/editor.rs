use agent_core::facade::{SkillFieldMappingView, SkillSourceView};
use crossterm::event::{KeyCode, KeyModifiers};

use crate::components::Command;

use super::state::{trim_option, SkillOverlayMode, SkillsOverlay};

const DEFAULT_SKILL_SEARCH_TEMPLATE: &str =
    "/api/skills?keyword={{query}}&page=1&pageSize={{limit}}&sortBy=downloads&order=desc";
const DEFAULT_SKILL_DOWNLOAD_TEMPLATE: &str = "/api/v1/download?slug={{slug}}";
const DEFAULT_SKILL_LIST_TEMPLATE: &str =
    "/api/skills?page=1&pageSize={{limit}}&sortBy=downloads&order=desc";
const DEFAULT_SKILL_DETAIL_TEMPLATE: &str = "/api/v1/skills/{{slug}}";
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SkillSourceEditorField {
    Id,
    DisplayName,
    Url,
    Kind,
    SearchTemplate,
    DownloadTemplate,
    ListTemplate,
    DetailTemplate,
    Priority,
    CacheTtlSeconds,
    Enabled,
}

pub(super) const SKILL_SOURCE_EDITOR_FIELDS: [SkillSourceEditorField; 11] = [
    SkillSourceEditorField::Id,
    SkillSourceEditorField::DisplayName,
    SkillSourceEditorField::Url,
    SkillSourceEditorField::Kind,
    SkillSourceEditorField::SearchTemplate,
    SkillSourceEditorField::DownloadTemplate,
    SkillSourceEditorField::ListTemplate,
    SkillSourceEditorField::DetailTemplate,
    SkillSourceEditorField::Priority,
    SkillSourceEditorField::CacheTtlSeconds,
    SkillSourceEditorField::Enabled,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SkillSourceDraft {
    pub(super) id: String,
    pub(super) display_name: String,
    pub(super) kind: String,
    pub(super) url: String,
    pub(super) search_template: String,
    pub(super) download_template: String,
    pub(super) list_template: String,
    pub(super) detail_template: String,
    pub(super) priority: String,
    pub(super) cache_ttl_seconds: String,
    pub(super) enabled: bool,
}

impl SkillSourceDraft {
    pub(super) fn new() -> Self {
        Self {
            id: String::new(),
            display_name: String::new(),
            kind: "skillhub".to_string(),
            url: String::new(),
            search_template: DEFAULT_SKILL_SEARCH_TEMPLATE.to_string(),
            download_template: DEFAULT_SKILL_DOWNLOAD_TEMPLATE.to_string(),
            list_template: DEFAULT_SKILL_LIST_TEMPLATE.to_string(),
            detail_template: DEFAULT_SKILL_DETAIL_TEMPLATE.to_string(),
            priority: "100".to_string(),
            cache_ttl_seconds: "900".to_string(),
            enabled: true,
        }
    }

    pub(super) fn to_view(&self) -> Option<SkillSourceView> {
        let id = self.id.trim();
        let display_name = self.display_name.trim();
        let kind = self.kind.trim();
        let url = self.url.trim();
        let search_template = self.search_template.trim();
        let download_template = self.download_template.trim();
        if id.is_empty()
            || display_name.is_empty()
            || kind.is_empty()
            || search_template.is_empty()
            || download_template.is_empty()
            || !(url.starts_with("http://") || url.starts_with("https://"))
        {
            return None;
        }

        Some(SkillSourceView {
            id: id.to_string(),
            display_name: display_name.to_string(),
            kind: kind.to_string(),
            url: url.to_string(),
            search_template: search_template.to_string(),
            download_template: download_template.to_string(),
            list_template: trim_option(&self.list_template),
            detail_template: trim_option(&self.detail_template),
            field_mapping: SkillFieldMappingView::default(),
            enabled: self.enabled,
            priority: self.priority.trim().parse::<u32>().unwrap_or(100),
            cache_ttl_seconds: self.cache_ttl_seconds.trim().parse::<u64>().unwrap_or(900),
            last_error: None,
        })
    }

    pub(super) fn push_char(&mut self, field: SkillSourceEditorField, ch: char) {
        match field {
            SkillSourceEditorField::Id => self.id.push(ch),
            SkillSourceEditorField::DisplayName => self.display_name.push(ch),
            SkillSourceEditorField::Kind => self.kind.push(ch),
            SkillSourceEditorField::Url => self.url.push(ch),
            SkillSourceEditorField::SearchTemplate => self.search_template.push(ch),
            SkillSourceEditorField::DownloadTemplate => self.download_template.push(ch),
            SkillSourceEditorField::ListTemplate => self.list_template.push(ch),
            SkillSourceEditorField::DetailTemplate => self.detail_template.push(ch),
            SkillSourceEditorField::Priority => {
                if ch.is_ascii_digit() {
                    self.priority.push(ch);
                }
            }
            SkillSourceEditorField::CacheTtlSeconds => {
                if ch.is_ascii_digit() {
                    self.cache_ttl_seconds.push(ch);
                }
            }
            SkillSourceEditorField::Enabled => match ch {
                ' ' => self.enabled = !self.enabled,
                'y' | 'Y' | '1' | 't' | 'T' => self.enabled = true,
                'n' | 'N' | '0' | 'f' | 'F' => self.enabled = false,
                _ => {}
            },
        }
    }

    pub(super) fn backspace(&mut self, field: SkillSourceEditorField) {
        match field {
            SkillSourceEditorField::Id => {
                self.id.pop();
            }
            SkillSourceEditorField::DisplayName => {
                self.display_name.pop();
            }
            SkillSourceEditorField::Kind => {
                self.kind.pop();
            }
            SkillSourceEditorField::Url => {
                self.url.pop();
            }
            SkillSourceEditorField::SearchTemplate => {
                self.search_template.pop();
            }
            SkillSourceEditorField::DownloadTemplate => {
                self.download_template.pop();
            }
            SkillSourceEditorField::ListTemplate => {
                self.list_template.pop();
            }
            SkillSourceEditorField::DetailTemplate => {
                self.detail_template.pop();
            }
            SkillSourceEditorField::Priority => {
                self.priority.pop();
            }
            SkillSourceEditorField::CacheTtlSeconds => {
                self.cache_ttl_seconds.pop();
            }
            SkillSourceEditorField::Enabled => {}
        }
    }

    pub(super) fn clear_field(&mut self, field: SkillSourceEditorField) {
        match field {
            SkillSourceEditorField::Id => self.id.clear(),
            SkillSourceEditorField::DisplayName => self.display_name.clear(),
            SkillSourceEditorField::Kind => self.kind.clear(),
            SkillSourceEditorField::Url => self.url.clear(),
            SkillSourceEditorField::SearchTemplate => self.search_template.clear(),
            SkillSourceEditorField::DownloadTemplate => self.download_template.clear(),
            SkillSourceEditorField::ListTemplate => self.list_template.clear(),
            SkillSourceEditorField::DetailTemplate => self.detail_template.clear(),
            SkillSourceEditorField::Priority => self.priority.clear(),
            SkillSourceEditorField::CacheTtlSeconds => self.cache_ttl_seconds.clear(),
            SkillSourceEditorField::Enabled => {}
        }
    }
}

impl SkillsOverlay {
    pub(super) fn current_source_field(&self) -> SkillSourceEditorField {
        SKILL_SOURCE_EDITOR_FIELDS[self.source_field_index]
    }

    pub(super) fn move_source_field_down(&mut self) {
        self.source_field_index = (self.source_field_index + 1) % SKILL_SOURCE_EDITOR_FIELDS.len();
    }

    pub(super) fn move_source_field_up(&mut self) {
        self.source_field_index = if self.source_field_index == 0 {
            SKILL_SOURCE_EDITOR_FIELDS.len() - 1
        } else {
            self.source_field_index - 1
        };
    }

    pub(super) fn handle_source_editor_key(
        &mut self,
        key: KeyCode,
        modifiers: KeyModifiers,
    ) -> Vec<Command> {
        match key {
            KeyCode::Tab | KeyCode::Down => self.move_source_field_down(),
            KeyCode::BackTab | KeyCode::Up => self.move_source_field_up(),
            KeyCode::Esc => self.mode = SkillOverlayMode::List,
            KeyCode::Backspace => self.source_draft.backspace(self.current_source_field()),
            KeyCode::Delete => self.source_draft.clear_field(self.current_source_field()),
            KeyCode::Enter => {
                if let Some(config) = self.source_draft.to_view() {
                    self.mode = SkillOverlayMode::List;
                    return vec![Command::AddSkillSource { config }];
                }
            }
            KeyCode::Char(ch) if !modifiers.contains(KeyModifiers::CONTROL) => {
                self.source_draft.push_char(self.current_source_field(), ch);
            }
            _ => {}
        }
        Vec::new()
    }
    pub(super) fn handle_catalog_filter_key(
        &mut self,
        key: KeyCode,
        modifiers: KeyModifiers,
    ) -> Vec<Command> {
        match key {
            KeyCode::Enter => {
                self.catalog_keyword = self.catalog_keyword_draft.trim().to_string();
                self.mode = SkillOverlayMode::List;
                return vec![self.list_catalog_command()];
            }
            KeyCode::Esc => {
                self.catalog_keyword_draft = self.catalog_keyword.clone();
                self.mode = SkillOverlayMode::List;
            }
            KeyCode::Backspace => {
                self.catalog_keyword_draft.pop();
            }
            KeyCode::Delete => {
                self.catalog_keyword_draft.clear();
            }
            KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.catalog_keyword_draft.clear();
            }
            KeyCode::Char(ch) if !modifiers.contains(KeyModifiers::CONTROL) => {
                self.catalog_keyword_draft.push(ch);
            }
            _ => {}
        }
        Vec::new()
    }
}
