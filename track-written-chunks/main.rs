use std::env;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{Seek, Read, SeekFrom, Write};

use std::path::Path;

use libc::ENOENT;
use fuse::{FileType, FileAttr, Filesystem, Request, ReplyData, ReplyEntry, ReplyAttr, ReplyDirectory};
use serde::{Serialize, Deserialize};
use time::Timespec;


struct HelloFS {
    file: std::fs::File,
    chunk_stats_file_name: String,
    chunk_stats: Vec<ChunkInfo>,
    size: u64
}

impl Filesystem for HelloFS {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        if parent == 1 && name.to_str() == Some("file.bin") {
            let one_second = Timespec::new(1, 0);
            reply.entry(&one_second, &FileAttr {
                ino: 2,
                size: self.size,
                blocks: self.size/512,
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
                size: self.size,
                blocks: self.size/512,
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
            // Seek to the given offset
            let mut file = &self.file;
            // If there are fewer than size bytes left in the file, read only the remaining bytes
            let true_size = size.min((self.size - offset as u64).try_into().unwrap());
            file.seek(std::io::SeekFrom::Start(offset as u64)).unwrap();
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
            let mut file = &self.file;

            // If the given offset is greater than the file, return ENOSPC
            if offset as u64 >= self.size {
                reply.error(libc::ENOSPC);
                return;
            }
            
            // Seek to the given offset
            file.seek(std::io::SeekFrom::Start(offset as u64)).unwrap();
            // Write the data that fits in the file
            let true_size = data.len().min((self.size - offset as u64).try_into().unwrap());

            // Get the chunks that have been written to
            let mut chunks = get_chunks_mut(&mut self.chunk_stats, offset as u64, true_size as u64);
            // Mark all chunks as written
            // If there are any that weren't marked as written, then write out the chunk stats file
            if chunks.iter().any(|chunk| !chunk.is_written) {
                for chunk in chunks.iter_mut() {
                    chunk.is_written = true;
                }
                // Write out the chunk stats file
                let mut chunk_stats_file = File::create(&self.chunk_stats_file_name).unwrap();
                chunk_stats_file.write_all(serde_json::to_string_pretty(&self.chunk_stats).unwrap().as_bytes()).unwrap();
            }

            file.write_all(&data[..true_size]).unwrap();
            reply.written(true_size as u32);


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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChunkInfo {
    begin_byte: u64,
    end_byte: u64,
    is_written: bool,
}

/// Given a vector of ChunkInfo structs and a byte offset and size,
/// return mutable references to the chunks that overlap with the given range.
fn get_chunks_mut(chunks: &mut Vec<ChunkInfo>, offset: u64, size: u64) -> Vec<&mut ChunkInfo> {
    let mut result = Vec::new();
    let end_byte = offset + size;
    // The chunks are known to be in order and of the same size.
    // Instead of iterating over all chunks, we can seek to the first chunk that overlaps with the range.
    let chunk_size = chunks[0].end_byte - chunks[0].begin_byte;
    let first_chunk_index = (offset / chunk_size).min(chunks.len() as u64 - 1) as usize;
    for chunk in chunks.iter_mut().skip(first_chunk_index) {
        if chunk.begin_byte > end_byte {
            break;
        }
        result.push(chunk);
    }
    result
}


impl ChunkInfo {
    /// Make multiple ChunkInfo structs that cover a range of bytes, with each struct covering chunk_size bytes.
    fn from_span(begin_byte: u64, end_byte: u64, chunk_size: u64) -> Vec<Self> {
        let mut chunks = Vec::new();
        let mut current_byte = begin_byte;
        while current_byte < end_byte {
            let chunk_end_byte = (current_byte + chunk_size).min(end_byte);
            chunks.push(ChunkInfo {
                begin_byte: current_byte,
                end_byte: chunk_end_byte,
                is_written: false,
            });
            current_byte = chunk_end_byte;
        }
        chunks
    }

    /// Verify that the given vector of ChunkInfo structs covers the given range of bytes
    /// without any gaps or overlaps,
    /// and that the chunks have a specific size.
    /// This size is then returned.
    fn verify_chunks(chunks: &[Self], begin_byte: u64, end_byte: u64) -> Result<u64, String> {
        if chunks.is_empty() {
            return Err("No chunks provided".to_string());
        }
        if chunks[0].begin_byte != begin_byte {
            return Err(format!("First chunk begins at byte {}, but expected {}", chunks[0].begin_byte, begin_byte));
        }
        if chunks[chunks.len() - 1].end_byte != end_byte {
            return Err(format!("Last chunk ends at byte {}, but expected {}", chunks[chunks.len() - 1].end_byte, end_byte));
        }
        let chunk_size = chunks[0].end_byte - chunks[0].begin_byte;
        for chunk in chunks {
            if chunk.end_byte - chunk.begin_byte != chunk_size {
                return Err(format!("Chunk at byte {} has size {}, but expected {}", chunk.begin_byte, chunk.end_byte - chunk.begin_byte, chunk_size));
            }
        }
        Ok(chunk_size)
    }
}

fn main() {
    env_logger::init();
    let mountpoint = env::args_os().nth(1).unwrap();
    let file = env::args_os().nth(2).unwrap();
    let wanted_chunk_size = env::args_os().nth(3).unwrap().to_str().unwrap().parse::<u64>().unwrap();
    let chunk_stats_file_name = env::args_os().nth(4).unwrap().to_str().unwrap().to_string();

    // Open provided file for reading and writing
    let mut file = File::options().read(true).write(true).open(file).unwrap();

    // Find the size of the file by seeking to the end
    let size = file.seek(SeekFrom::End(0)).unwrap();

    let used_chunk_size;

    // Load chunk stats from file
    let chunk_stats = if Path::new(&chunk_stats_file_name).exists() {
        let chunk_stats_file = File::open(&chunk_stats_file_name).unwrap();
        let chunk_stats: Vec<ChunkInfo> = serde_json::from_reader(chunk_stats_file).unwrap();
        used_chunk_size = ChunkInfo::verify_chunks(&chunk_stats, 0, size).expect("Chunk stats file is invalid");
        chunk_stats
    } else {
        // If the file doesn't exist, make a new one
        let chunk_stats = ChunkInfo::from_span(0, size, wanted_chunk_size);
        let chunk_stats_file = File::create(&chunk_stats_file_name).unwrap();
        serde_json::to_writer_pretty(chunk_stats_file, &chunk_stats).unwrap();
        used_chunk_size = wanted_chunk_size;
        chunk_stats
    };

    println!("Chunk size: {}", used_chunk_size);



    let options = ["-o", "fsname=hello"]
        .iter()
        .map(|o| o.as_ref())
        .collect::<Vec<&OsStr>>();
    fuse::mount(HelloFS{
        file, chunk_stats_file_name, chunk_stats, size
    }, &mountpoint, &options).unwrap();
}
