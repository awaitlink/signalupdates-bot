use super::Language;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalizationChange<'a> {
    pub language: Language,
    pub filename: &'a str,
}

impl LocalizationChange<'_> {
    #[cfg(test)]
    pub fn default_for_android() -> LocalizationChange<'static> {
        LocalizationChange {
            language: Default::default(),
            filename: crate::platform::ANDROID_DEFAULT_STRINGS_FILENAME,
        }
    }
}
