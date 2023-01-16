use std::ffi::{OsStr, c_int};

use libc::{ENOSYS, ENOENT, ENOTDIR};
use trace::trace;

use log::*;

trace::init_depth_var!();


use fuse::{Filesystem, FileType};
use time::Timespec;

use crate::{handler::{DirectoryListing, PathHandler}, identity::ItemIdentity};
use crate::handler::Identifiable;
pub struct RoutableFilesystem<'a> {
    ino_to_path: std::collections::HashMap<u64, String>,
    path_to_ino: std::collections::HashMap<String, u64>,
    ino_to_handler: std::collections::HashMap<u64, PathHandler<'a>>,
    ino_parent: std::collections::HashMap<u64, u64>,
    identity_to_ino: std::collections::HashMap<ItemIdentity, u64>,
    latest_ino: u64,

    root: DirectoryListing<'a>,
}

impl<'a> RoutableFilesystem<'a> {
    fn get_ino_by_identity(&mut self, identity: ItemIdentity) -> u64 {
        if let Some(ino) = self.identity_to_ino.get(&identity) {
            *ino
        } else {
            self.identity_to_ino.insert(identity, self.latest_ino);
            self.latest_ino += 1;
            self.latest_ino - 1
        }
    }
}

impl<'a> Filesystem for RoutableFilesystem<'a> {
    #[trace]
    fn init(&mut self, _req: &fuse::Request) -> Result<(), c_int> {
        Ok(())
    }

    fn readdir(&mut self, _req: &fuse::Request, ino: u64, _fh: u64, offset: i64, mut reply: fuse::ReplyDirectory) {
        // If the ino doesn't have a handler, return ENOENT
        if !self.ino_to_handler.contains_key(&ino) {
            reply.error(ENOENT);
            return;
        }

        // If the ino has a handler, but it's not a directory, return ENOTDIR
        let dir_handler = match self.ino_to_handler.get(&ino).unwrap() {
            PathHandler::Directory(handler) => handler.clone(),
            _ => {
                reply.error(ENOTDIR);
                return;
            }
        };

        let parent_ino = self.ino_parent.get(&ino).unwrap();

        let mut entries = vec![
            (ino, FileType::Directory, ".".to_string()),
            (*parent_ino, FileType::Directory, "..".to_string()),
        ];

        for (name, handler) in dir_handler.listdir() {
            let ino = self.get_ino_by_identity(handler.get_identity());
            entries.push((ino, handler.get_type(), name));
        }

        for (i, (ino, ty, name)) in entries.iter().enumerate().skip(offset as usize) {
            if reply.add(*ino, (i + 1) as i64, *ty, OsStr::new(name)) {
                break;
            }
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

        let handler = match self.ino_to_handler.get(&ino) {
            Some(handler) => handler,
            None => {
                info!("getattr: no handler for ino {}", ino);
                reply.error(ENOENT);
                return;
            }
        };

        match handler {
            PathHandler::Directory(_) => {
                attr.size = 4096;
                attr.blocks = 8;
                attr.kind = FileType::Directory;
                attr.perm = 0o755;
                attr.nlink = 2;
            },
            PathHandler::File(file) => {
                attr.size = file.get_size();
                attr.blocks = file.get_size() / 512;
                attr.kind = FileType::RegularFile;
                attr.perm = 0o755;
                attr.nlink = 1;
            }
        }
        reply.attr(&Timespec::new(0, 0), &attr);

    }

    fn lookup(&mut self, _req: &fuse::Request, parent: u64, name: &OsStr, reply: fuse::ReplyEntry) {
        // Check if the parent is a directory
        if !self.ino_to_handler.contains_key(&parent) {
            info!("lookup: no handler for parent ino {parent}");
            reply.error(ENOENT);
            return;
        }
        let dirhandler = match self.ino_to_handler.get(&parent).unwrap() {
            PathHandler::Directory(handler) => handler.clone(),
            _ => {
                reply.error(ENOTDIR);
                return;
            }
        };

        // Get the directory listing and find the item's identity
        let listing: Vec<(String, &PathHandler)> = dirhandler.listdir();
        let handler = match listing.iter().find(|(n, _)| n == name.to_str().unwrap()) {
            Some((_, handler)) => handler,
            None => {
                reply.error(ENOENT);
                return;
            }
        };
        let attrs = match handler {
            PathHandler::File(fhandler) => {
                let ino = self.get_ino_by_identity(handler.get_identity());
                fuse::FileAttr {
                    ino: ino,
                    size: fhandler.get_size(),
                    blocks: fhandler.get_size() / 512,
                    atime: Timespec::new(0, 0),
                    mtime: Timespec::new(0, 0),
                    ctime: Timespec::new(0, 0),
                    crtime: Timespec::new(0, 0),
                    kind: FileType::RegularFile,
                    perm: 0o755,
                    nlink: 0,
                    uid: 0,
                    gid: 0,
                    rdev: 0,
                    flags: 0,
                }
            },
            PathHandler::Directory(dhandler) => fuse::FileAttr {
                ino: self.get_ino_by_identity(handler.get_identity()),
                size: 4096,
                blocks: 8,
                atime: Timespec::new(0, 0),
                mtime: Timespec::new(0, 0),
                ctime: Timespec::new(0, 0),
                crtime: Timespec::new(0, 0),
                kind: FileType::Directory,
                perm: 0o755,
                nlink: 2,
                uid: 0,
                gid: 0,
                rdev: 0,
                flags: 0,
            },
        };

        reply.entry(&Timespec::new(0, 0), &attrs, 0);
        
    }

}

impl<'a> RoutableFilesystem<'a> {
    pub fn new() -> RoutableFilesystem<'a> {
        let mut parents = std::collections::HashMap::new();
        let ino_to_handler = std::collections::HashMap::new();
        let mut ino_to_path = std::collections::HashMap::new();
        let mut path_to_ino = std::collections::HashMap::new();

        parents.insert(1, 1); // root's parent is root
        ino_to_path.insert(1, "/".to_string());
        path_to_ino.insert("/".to_string(), 1);

        RoutableFilesystem {
            ino_to_path,
            path_to_ino,
            ino_to_handler,
            ino_parent: parents,
            root: DirectoryListing::new(),
            identity_to_ino: std::collections::HashMap::new(),
            latest_ino: 2,
        }
    }

    pub fn set_root(&mut self, root: DirectoryListing<'a>) {
        self.root = root;
        self.ino_to_handler.insert(1, PathHandler::Directory(self.root.clone()));
        self.ino_to_path.insert(1, "/".to_string());
        self.path_to_ino.insert("/".to_string(), 1);
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