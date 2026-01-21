use crate::Oid;
use std::vec;
use std::{
    fs::File,
    io::{self, Write},
    path::Path,
};
pub struct Index {
    pub entries: Vec<IndexEntry>,
}
pub struct IndexEntry {
    pub ctime_secs: u32,
    pub ctime_nsec: u32,
    pub mtime_secs: u32,
    pub mtime_nsec: u32,
    pub dev: u32,
    pub ino: u32,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub file_size: u32,
    pub oid: Oid,
    pub flags: u16,
    pub path: String,
}

impl IndexEntry {
    pub fn write<W: Write>(&self, w: &mut W) -> io::Result<()> {
        w.write_all(&self.ctime_secs.to_be_bytes())?;
        w.write_all(&self.ctime_nsec.to_be_bytes())?;
        w.write_all(&self.mtime_secs.to_be_bytes())?;
        w.write_all(&self.mtime_nsec.to_be_bytes())?;
        w.write_all(&self.dev.to_be_bytes())?;
        w.write_all(&self.ino.to_be_bytes())?;
        w.write_all(&self.mode.to_be_bytes())?;
        w.write_all(&self.uid.to_be_bytes())?;
        w.write_all(&self.gid.to_be_bytes())?;
        w.write_all(&self.file_size.to_be_bytes())?;
        w.write_all(&self.oid)?;
        w.write_all(&self.flags.to_be_bytes())?;
        w.write_all(&self.path.as_bytes())?;
        w.write_all(&[0])?;
        let entry_len = 62 + &self.path.as_bytes().len() + 1;
        let pad_len = (8 - (entry_len % 8)) % 8;
        let pad = vec![b'\0'; pad_len];
        w.write_all(&pad)?;
        Ok(())
    }
}

impl Index {
    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let mut file = File::options().append(true).create(true).open(&path)?;
        for entry in &self.entries {
            entry.write(&mut file)?;
        }
        Ok(())
    }

    pub fn sort(&mut self) {
        self.entries
            .sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    }
}
