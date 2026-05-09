pub mod frontmatter;
pub mod types;

#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("missing SKILL.md frontmatter")]
    MissingFrontmatter,
    #[error("missing required frontmatter field: {field}")]
    MissingRequiredField { field: &'static str },
    #[error("invalid SKILL.md frontmatter: {0}")]
    InvalidFrontmatter(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, SkillError>;
