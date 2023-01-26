use std::env;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{Seek, Read, Write};
use libc::ENOENT;
use fuse::{FileType, FileAttr, Filesystem, Request, ReplyData, ReplyEntry, ReplyAttr, ReplyDirectory};
use time::Timespec;

const PART_SIZE: u64 = 1024*1024; // 1MB

// The inos are assigned as follows:
// Ino 1 is the root directory
// Ino 2 is the chunk from zero to PART_SIZE bytes (has name "0.part")
//

struct HelloFS {
    file: std::fs::File,
    file_size: u64,
    handle_lock: std::sync::Mutex<()>,
}

impl HelloFS {
    fn lookup_piece_info(&self, text_name: &str) -> Option<FileAttr> {
        println!("Looking up piece info for {text_name:?}");
        let name_number = text_name.split('.').find(|_| true).expect("File's name has less than zero dots?").parse::<u64>();
        if name_number.is_err() {
            return None;
        }
        let name_number = name_number.unwrap();
        let file_size = self.file_size;
        
        // If requested part is strictly after the end of the file, return ENOENT
        if name_number*PART_SIZE >= file_size {
            println!("Requested part is strictly after the end of the file ({name_number}*{PART_SIZE} = {} >= {file_size})", name_number*PART_SIZE);
            return None;
        }
        let size;
        // If requested part is the last one, return its size as the size of the file
        if (name_number+1)*PART_SIZE >= file_size {
            size = file_size - name_number*PART_SIZE;
        } else {
            size = PART_SIZE;
        }


        let one_second = Timespec::new(1, 0);
        Some(FileAttr {
            ino: name_number + 2,
            size: size,
            blocks: size/512,
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
        })
    }
}

impl Filesystem for HelloFS {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        if parent!=1 {
            reply.error(ENOENT);
            return;
        }
        if let Some(text_name) = name.to_str() {
            if let Some(attr) = self.lookup_piece_info(text_name) {
                let one_second = Timespec::new(1, 0);
                reply.entry(&one_second, &attr, 0);
                return;
            } else {
                reply.error(ENOENT);
                return;
            }
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
            other => {
                if let Some(attr) = self.lookup_piece_info(&(other+2).to_string()) {
                    reply.attr(&one_second, &attr);
                } else {
                    reply.error(ENOENT);
                }
            }
        }
    }

    fn read(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, size: u32, reply: ReplyData) {
        let _guard = self.handle_lock.lock().unwrap();
        let chunk_number = ino - 1;
        let mut buf = vec![0; size as usize];
        // Seek first to the beginning of the chunk, then to the given offset
        let absolute_offset = (chunk_number - 1)*PART_SIZE + offset as u64;


        if self.file.seek(std::io::SeekFrom::Start(absolute_offset)).is_err() {
            reply.error(libc::EINVAL);
            return;
        }
        let result = self.file.read(&mut buf);
        match result {
            Ok(size) => reply.data(&buf[..size]),
            Err(_) => reply.error(libc::EINVAL),
        }
    }

    fn open(&mut self, _req: &Request, _ino: u64, flags: u32, reply: fuse::ReplyOpen) {
        // This filesystem does not need to make file handles: writes are assumed stateless
        reply.opened(0, flags);
    }

    fn setattr(&mut self, _req: &Request, ino: u64, _mode: Option<u32>, _uid: Option<u32>, _gid: Option<u32>, _size: Option<u64>, _atime: Option<Timespec>, _mtime: Option<Timespec>, _fh: Option<u64>, _crtime: Option<Timespec>, _chgtime: Option<Timespec>, _bkuptime: Option<Timespec>, _flags: Option<u32>, reply: ReplyAttr) {
        // The user cannot change attributes here
        // Just respond with the old attributes for the file.

        let old_attr = self.lookup_piece_info(&format!("{}.part", ino-2));
        let one_second = Timespec::new(1, 0);
        match old_attr {
            Some(attr) => reply.attr(&one_second, &attr),
            None => todo!(),
        }
    }

    fn write(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, data: &[u8], _flags: u32, reply: fuse::ReplyWrite) {
        let _guard = self.handle_lock.lock().unwrap();
        // Seek first to the beginning of the chunk, then to the given offset
        let chunk_number = ino - 1;
        let absolute_offset = (chunk_number - 1)*PART_SIZE + offset as u64;
        println!("Write {} at {offset} at chunk {chunk_number} (absolute {absolute_offset})", data.len());

        // If the "where" to seek is beyond the scope of this chunk, return an error.
        if offset as u64 > PART_SIZE {
            reply.error(libc::ENOSPC);
            return;
        }
        if self.file.seek(std::io::SeekFrom::Start(absolute_offset)).is_err() {
            println!("Error seeking to {absolute_offset}");
            reply.error(libc::ENOSPC);
            return;
        }

        let result = self.file.write(data);
        match result {
            Ok(size) => reply.written(size as u32),
            Err(e) => {
                println!("Error writing: {e:?}");
                reply.error(libc::EINVAL)
            },
        }
    }

    fn readdir(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, mut reply: ReplyDirectory) {
        if ino != 1 {
            reply.error(ENOENT);
            return;
        }

        let mut entries = vec![
            (1, FileType::Directory, ".".to_string()),
            (1, FileType::Directory, "..".to_string()),
        ];

        let file_size = self.file_size;
        let number_of_parts = file_size / PART_SIZE;
        for i in 0..number_of_parts {
            let name = format!("{}.part", i);
            entries.push((i+1, FileType::RegularFile, name));
        }

        for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
            // i + 1 means the index of the next entry
            reply.add(entry.0, (i + 2) as i64, entry.1, entry.2);
        }
        reply.ok();
    }
}

fn main() {
    env_logger::init();
    let mountpoint = env::args_os().nth(1).unwrap();
    let file = env::args_os().nth(2).unwrap();
    // Open provided file for reading and writing
    let mut file = File::options().read(true).write(true).open(file).unwrap();

    // Find file size by seeking to the end
    let file_size = file.seek(std::io::SeekFrom::End(0)).unwrap();
    let options = ["-o", "fsname=hello"]
        .iter()
        .map(|o| o.as_ref())
        .collect::<Vec<&OsStr>>();
    
    let handle_lock = std::sync::Mutex::new(());

    fuse::mount(HelloFS{
        file, file_size, handle_lock
    }, &mountpoint, &options).unwrap();
}
