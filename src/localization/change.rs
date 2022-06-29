use super::Language;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalizationChange {
    pub language: Language,
    pub filename: String,
}

impl LocalizationChange {
    pub fn default_for_android() -> LocalizationChange {
        LocalizationChange {
            language: Default::default(),
            filename: crate::platform::ANDROID_DEFAULT_STRINGS_FILENAME.to_owned(),
        }
    }
}
