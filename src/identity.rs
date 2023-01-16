use rand::Rng;

/// A unique identifier for an item (file or directory) in a file system.
/// 
/// This is used to create a unique inode number for each item in the file system.
/// 
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ItemIdentity(u64);

impl ItemIdentity {
    /// Create a new identity.
    /// 
    /// This is used to create a unique inode number for each item in the file system.
    /// 
    pub fn new() -> Self {
        let mut rng = rand::thread_rng();
        let id = rng.gen::<u64>();
        ItemIdentity(id)
    }
}