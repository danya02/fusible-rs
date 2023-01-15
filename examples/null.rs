/// A null filesystem. All operations return ENOSYS.

use fusible::RoutableFilesystem;


fn main() {
    env_logger::init();

    let mountpoint = std::env::args().nth(1).expect("missing mountpoint argument");
    let fs = RoutableFilesystem::new();
    fs.mount(&mountpoint);
}