use crate::DEBUG;
use memmap2::Mmap;
use std::fs::File;
use std::io;
use std::io::{BufReader, Cursor, Read, Seek};
use std::path::Path;
use zip::ZipArchive;

/// For some reason reading the zip through WSL is extremely slow,
/// so we use a memory mapped file as an intermediary.
///
/// Falls back to reading it normally if mapping fails.
pub fn open_zip(path: &Path) -> Option<ZipArchive<ZipSource>> {
    match File::open(path) {
        Ok(file) => {
            let mmap = unsafe { Mmap::map(&file) };
            match mmap {
                Ok(mmap) => {
                    let zip = Cursor::new(mmap);
                    match ZipArchive::new(ZipSource::Mapped(zip)) {
                        Ok(zip) => {
                            return Some(zip);
                        }
                        Err(e) => {
                            if DEBUG {
                                println!("Failed to open zip: {}", e);
                            }
                            return None;
                        }
                    }
                }
                Err(e) => {
                    if DEBUG {
                        println!("Failed to memory map zip: {}", e);
                    }

                    match ZipArchive::new(ZipSource::Buffered(BufReader::new(file))) {
                        Ok(zip) => {
                            return Some(zip);
                        }
                        Err(e) => {
                            if DEBUG {
                                println!("Failed to open zip: {}", e);
                            }
                            return None;
                        }
                    }
                }
            }
        }
        Err(e) => {
            if DEBUG {
                println!("Failed to open zip: {}", e);
            }
            return None;
        }
    }
}

#[derive(Debug)]
pub enum ZipSource {
    Mapped(Cursor<Mmap>),
    File(File),
    Buffered(BufReader<File>),
}

impl Read for ZipSource {
    #[inline(always)]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            ZipSource::Mapped(cursor) => cursor.read(buf),
            ZipSource::File(file) => file.read(buf),
            ZipSource::Buffered(buffered) => buffered.read(buf),
        }
    }
}

impl Seek for ZipSource {
    #[inline(always)]
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        match self {
            ZipSource::Mapped(cursor) => cursor.seek(pos),
            ZipSource::File(file) => file.seek(pos),
            ZipSource::Buffered(buffered) => buffered.seek(pos),
        }
    }
}