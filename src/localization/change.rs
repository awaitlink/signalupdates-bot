use super::Language;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalizationChange {
    pub language: Language,
    pub filename: String,
}
