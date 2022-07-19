pub enum CommitStatus {
    Both {
        reverts: usize,
        is_reverted_by: usize,
    },
    IsRevertedBy(usize),
    Reverts(usize),
    Normal,
}
