use super::Language;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalizationChange<'a> {
    pub language: Language,
    pub filename: &'a str,
}
