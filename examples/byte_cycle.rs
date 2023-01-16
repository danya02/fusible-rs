/// A file system containing a single file of a large size.
/// The file is filled with a repeating pattern of bytes, from 0 to 255.

use fusible::{RoutableFilesystem, handler::{DirectoryListing, FileHandler}};

#[derive(Debug, Clone)]
struct ByteCycle {
    size: u64,
}

impl ByteCycle {
    pub fn new(size: u64) -> ByteCycle {
        ByteCycle { size }
    }
}

impl FileHandler for ByteCycle {

    fn get_size(&self) -> u64 {
        self.size
    }
}

fn main() {
    env_logger::init();

    let mountpoint = std::env::args().nth(1).expect("missing mountpoint argument");
    let mut fs = RoutableFilesystem::new();

    let root = DirectoryListing::new()
        .add_file("cycle", ByteCycle::new(1024 * 1024 * 1024));
    
    fs.set_root(root);

    fs.mount(&mountpoint);
}