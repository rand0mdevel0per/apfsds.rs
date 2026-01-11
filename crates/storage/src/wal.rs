use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use crc32fast::Hasher;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Write-Ahead Log (WAL) entry header size (CRC32 + Length)
#[allow(dead_code)]
const HEADER_SIZE: usize = 4 + 8;

/// Write-Ahead Log for persistent storage
pub struct Wal {
    #[allow(dead_code)]
    path: PathBuf,
    file: Arc<Mutex<File>>,
}

impl Wal {
    /// Open or create a WAL file
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)?;

        Ok(Self {
            path,
            file: Arc::new(Mutex::new(file)),
        })
    }

    /// Append an entry to the WAL
    pub fn append(&self, data: &[u8]) -> io::Result<u64> {
        let mut file = self.file.lock().unwrap();

        // Calculate CRC32
        let mut hasher = Hasher::new();
        hasher.update(data);
        let checksum = hasher.finalize();

        // Write header: CRC32 (4 bytes) + Length (8 bytes)
        file.write_u32::<BigEndian>(checksum)?;
        file.write_u64::<BigEndian>(data.len() as u64)?;

        // Write data
        file.write_all(data)?;

        Ok(file.stream_position()?)
    }

    /// Sync changes to disk
    pub fn sync(&self) -> io::Result<()> {
        let file = self.file.lock().unwrap();
        file.sync_all()
    }

    /// Truncate the WAL to a specific size
    pub fn truncate(&self, size: u64) -> io::Result<()> {
        let file = self.file.lock().unwrap();
        file.set_len(size)?;
        file.sync_all()
    }

    /// Read all entries from the WAL
    pub fn read_all(&self) -> io::Result<Vec<Vec<u8>>> {
        let mut file = self.file.lock().unwrap();
        file.seek(SeekFrom::Start(0))?;

        let mut entries = Vec::new();
        let mut buffer = Vec::new();

        loop {
            // Read header
            let checksum = match file.read_u32::<BigEndian>() {
                Ok(c) => c,
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e),
            };

            let len = file.read_u64::<BigEndian>()?;

            // Validate length sanity check (max 128MB per entry)
            if len > 128 * 1024 * 1024 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Entry too large",
                ));
            }

            // Read data
            buffer.resize(len as usize, 0);
            file.read_exact(&mut buffer)?;

            // Verify checksum
            let mut hasher = Hasher::new();
            hasher.update(&buffer);
            if hasher.finalize() != checksum {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Checksum mismatch",
                ));
            }

            entries.push(buffer.clone());
        }

        // Restore file position to end
        file.seek(SeekFrom::End(0))?;

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_wal_persistence() -> io::Result<()> {
        let temp_file = NamedTempFile::new()?;
        let path = temp_file.path().to_path_buf();

        // Write data
        {
            let wal = Wal::open(&path)?;
            wal.append(b"hello")?;
            wal.append(b"world")?;
            wal.sync()?;
        }

        // Read back
        {
            let wal = Wal::open(&path)?;
            let entries = wal.read_all()?;
            assert_eq!(entries.len(), 2);
            assert_eq!(entries[0], b"hello");
            assert_eq!(entries[1], b"world");
        }

        Ok(())
    }
}
