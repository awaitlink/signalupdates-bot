use strum_macros::EnumIter;

#[derive(Debug, EnumIter, Clone, Copy)]
pub enum LocalizationChangeRenderMode {
    Full,
    WithoutRelease,
    Nothing,
}
