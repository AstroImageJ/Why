use std::fs::File;
use std::io::{self, BufReader, Cursor, Read, Seek};
use std::path::Path;
use memmap2::Mmap;
use zip::ZipArchive;
use crate::DEBUG;

/// A wrapper for different types of zip file sources
#[derive(Debug)]
pub enum ZipSource {
    Mapped(Cursor<Mmap>),
    Buffered(BufReader<File>),
    File(File),
}

impl Read for ZipSource {
    #[inline(always)]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            ZipSource::Mapped(cursor) => cursor.read(buf),
            ZipSource::Buffered(reader) => reader.read(buf),
            ZipSource::File(file) => file.read(buf),
        }
    }
}

impl Seek for ZipSource {
    #[inline(always)]
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        match self {
            ZipSource::Mapped(cursor) => cursor.seek(pos),
            ZipSource::Buffered(reader) => reader.seek(pos),
            ZipSource::File(file) => file.seek(pos),
        }
    }
}

/// For some reason reading the zip through WSL is extremely slow,
/// so we use a memory mapped file as an intermediary.
///
/// Falls back to reading it normally if mapping fails.
pub fn open_zip(path: &Path) -> Option<ZipArchive<ZipSource>> {
    File::open(path).ok().and_then(|file| {
        // Try memory mapping first
        match unsafe { Mmap::map(&file) } {
            Ok(mmap) => {
                let source = ZipSource::Mapped(Cursor::new(mmap));
                ZipArchive::new(source).ok()
            }
            Err(err) => {
                if DEBUG {
                    eprintln!("Failed to memmap zip file: {}", err);
                }

                // Fall back to buffered reading
                let source = ZipSource::Buffered(BufReader::new(file));
                ZipArchive::new(source).ok()
            }
        }
    })
}