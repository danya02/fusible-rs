use std::env;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{Seek, Read};
use std::os::unix::prelude::FileExt;

use libc::ENOENT;
use fuse::{FileType, FileAttr, Filesystem, Request, ReplyData, ReplyEntry, ReplyAttr, ReplyDirectory};
use time::Timespec;


struct HelloFS {
    file: std::fs::File,
    begin_byte: u64,
    end_byte: u64,
}

impl HelloFS {
    fn size(&self) -> u64 {
        self.end_byte - self.begin_byte
    }
}

impl Filesystem for HelloFS {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        if parent == 1 && name.to_str() == Some("file.bin") {
            let one_second = Timespec::new(1, 0);
            reply.entry(&one_second, &FileAttr {
                ino: 2,
                size: self.size(),
                blocks: self.size()/512,
                atime: one_second,                                  // 1970-01-01 00:00:00
                mtime: one_second,
                ctime: one_second,
                crtime: one_second,
                kind: FileType::RegularFile,
                perm: 0o644,
                nlink: 1,
                uid: 501,
                gid: 20,
                rdev: 0,
                flags: 0,
            }, 0);
        } else {
            reply.error(ENOENT);
        }
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        let one_second = Timespec::new(1, 0);
        match ino {
            1 => reply.attr(&one_second, &FileAttr {
                ino: 1,
                size: 0,
                blocks: 0,
                atime: one_second,                                  // 1970-01-01 00:00:00
                mtime: one_second,
                ctime: one_second,
                crtime: one_second,
                kind: FileType::Directory,
                perm: 0o755,
                nlink: 2,
                uid: 501,
                gid: 20,
                rdev: 0,
                flags: 0,
            }),
            2 => reply.attr(&one_second, &FileAttr {
                ino: 2,
                size: self.size(),
                blocks: self.size()/512,
                atime: one_second,                                  // 1970-01-01 00:00:00
                mtime: one_second,
                ctime: one_second,
                crtime: one_second,
                kind: FileType::RegularFile,
                perm: 0o644,
                nlink: 1,
                uid: 501,
                gid: 20,
                rdev: 0,
                flags: 0,
            }),
            _ => reply.error(ENOENT),
        }
    }

    fn read(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, size: u32, reply: ReplyData) {
        if ino == 2 {
            let mut file = &self.file;
            let true_offset = offset as u64 + self.begin_byte;
            let expected_end_byte = true_offset + size as u64;
            let true_end_byte = expected_end_byte.min(self.end_byte);
            let true_size = true_end_byte - true_offset;
            // Seek to offset
            file.seek(std::io::SeekFrom::Start(true_offset as u64)).unwrap();
            // Read size bytes
            let mut buf = vec![0; true_size as usize];
            file.read_exact(&mut buf).unwrap();
            reply.data(&buf);
        } else {
            reply.error(ENOENT);
        }
    }

    fn write(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, data: &[u8], _flags: u32, reply: fuse::ReplyWrite) {
        if ino == 2 {
            let file = &self.file;
            let true_offset = offset as u64 + self.begin_byte;
            if true_offset > self.end_byte {
                reply.error(libc::ENOSPC);
                return;
            }

            let expected_end_byte = true_offset + data.len() as u64;
            let true_end_byte = expected_end_byte.min(self.end_byte);
            let true_size = true_end_byte - true_offset;
            let result = file.write_all_at(&data[..true_size as usize], true_offset);
            match result {
                Ok(_) => reply.written(true_size as u32),
                Err(err) => {
                    println!("Error writing {true_size} bytes to offset {offset} which is true offset {true_offset}: {err:?}");
                    reply.error(libc::EIO);
                },
            }

        } else {
            reply.error(ENOENT);
        }

    }


    fn readdir(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, mut reply: ReplyDirectory) {
        if ino != 1 {
            reply.error(ENOENT);
            return;
        }

        let entries = vec![
            (1, FileType::Directory, "."),
            (1, FileType::Directory, ".."),
            (2, FileType::RegularFile, "file.bin"),
        ];

        for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
            // i + 1 means the index of the next entry
            reply.add(entry.0, (i + 1) as i64, entry.1, entry.2);
        }
        reply.ok();
    }


    fn open(&mut self, _req: &Request, _ino: u64, flags: u32, reply: fuse::ReplyOpen) {
        // This filesystem does not need to make file handles: writes are assumed stateless
        reply.opened(0, flags);
    }

    fn setattr(&mut self, req: &Request, ino: u64, _mode: Option<u32>, _uid: Option<u32>, _gid: Option<u32>, _size: Option<u64>, _atime: Option<Timespec>, _mtime: Option<Timespec>, _fh: Option<u64>, _crtime: Option<Timespec>, _chgtime: Option<Timespec>, _bkuptime: Option<Timespec>, _flags: Option<u32>, reply: ReplyAttr) {
        // The user cannot change attributes here
        // Just respond with the old attributes for the file.

        self.getattr(req, ino, reply);
    }

}

fn main() {
    env_logger::init();
    let mountpoint = env::args_os().nth(1).unwrap();
    let file = env::args_os().nth(2).unwrap();
    let begin_byte = env::args_os().nth(3).unwrap().to_str().unwrap().parse::<u64>().unwrap();
    let end_byte = env::args_os().nth(4).unwrap().to_str().unwrap().parse::<u64>().unwrap();
    assert!(end_byte > begin_byte);
    // Open provided file for reading and writing
    let file = File::options().read(true).write(true).open(file).unwrap();

    let options = ["-o", "fsname=hello"]
        .iter()
        .map(|o| o.as_ref())
        .collect::<Vec<&OsStr>>();
    fuse::mount(HelloFS{
        file, begin_byte, end_byte,
    }, &mountpoint, &options).unwrap();
}
