use std::ffi::{OsStr, c_int};

use libc::{ENOSYS, ENOENT};
use trace::trace;

trace::init_depth_var!();


use fuse::{Filesystem, FileType};
use time::Timespec;
pub struct RoutableFilesystem {
    ino_to_path: std::collections::HashMap<u64, String>,
    path_to_ino: std::collections::HashMap<String, u64>,
}

impl Filesystem for RoutableFilesystem {
    #[trace]
    fn init(&mut self, _req: &fuse::Request) -> Result<(), c_int> {
        Ok(())
    }

    fn readdir(&mut self, _req: &fuse::Request, ino: u64, _fh: u64, offset: i64, mut reply: fuse::ReplyDirectory) {
        if ino > 1 {
            reply.error(ENOENT);
            return;
        }

        let entries = vec![
            (1, FileType::Directory, "."),
            (1, FileType::Directory, ".."),
        ];

        for(i, (ino, kind, name)) in entries.into_iter().enumerate().skip(offset as usize) {
            reply.add(ino, (i+1) as i64, kind, name);
        }
        reply.ok();
        
    }

    #[trace]
    fn getattr(&mut self, _req: &fuse::Request, ino: u64, reply: fuse::ReplyAttr) {
        let mut attr = fuse::FileAttr {
            ino: ino,
            size: 0,
            blocks: 0,
            atime: Timespec::new(0, 0),
            mtime: Timespec::new(0, 0),
            ctime: Timespec::new(0, 0),
            crtime: Timespec::new(0, 0),
            kind: FileType::Directory,
            perm: 0o755,
            nlink: 0,
            uid: 0,
            gid: 0,
            rdev: 0,
            flags: 0,
        };

        if ino == 1 {
            attr.size = 4096;
            attr.blocks = 8;
            attr.kind = FileType::Directory;
            attr.perm = 0o755;
            attr.nlink = 2;
        } else {
            reply.error(ENOENT);
            return;
        }

        reply.attr(&Timespec::new(0, 0), &attr);

    }

}

impl RoutableFilesystem {
    pub fn new() -> RoutableFilesystem {
        RoutableFilesystem {
            ino_to_path: std::collections::HashMap::new(),
            path_to_ino: std::collections::HashMap::new(),
        }
    }


    /// Mount the filesystem at the given path
    /// with sensible defaults.
    /// 
    /// If you want to customize the mount options,
    /// use [`fuse::mount`] instead.
    pub fn mount(self, path: &str) -> () {
        let options = ["-o", "ro"]
        .iter()
        .map(|o| o.as_ref())
        .collect::<Vec<&OsStr>>();
        fuse::mount(self, &path, &options).unwrap();
    }
}