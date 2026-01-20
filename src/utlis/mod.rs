use std::fs::{File, create_dir_all, rename};
use std::io::{self, BufRead, BufReader, ErrorKind, prelude::*};
use std::{
    error::Error,
    fs::{self},
    io::Write,
    path::{Path, PathBuf},
};

// use std::{env, path};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, PartialEq)]
pub enum ObjType {
    Blob,
    Commit,
    Tag,
    Tree,
}
pub fn get_absolute_path<P: AsRef<Path>>(path: P) -> io::Result<PathBuf> {
    let input_path = path.as_ref();

    // canonicalize converts relative path → absolute path
    // and also resolves symbolic links, `.` and `..`
    let abs_path = input_path.canonicalize()?;

    Ok(abs_path)
}

pub fn write_content_atomically(path: &Path, content: &[u8]) -> Result<(), Box<dyn Error>> {
    let file_path = get_absolute_path(path)?;
    let parent = file_path.parent();
    let parent_path = match parent {
        Some(parent_path) => parent_path,
        None => return Err("Path has no parent directory".into()),
    };

    if !parent_path.try_exists()? {
        return Err("Parent directory doesn't exist".into());
    }

    let tmp = parent_path.join("tmp");

    let mut file = File::create(&tmp)?;
    file.write_all(content)?;
    file.sync_all()?;

    rename(tmp, &file_path)?;

    let dir = File::open(parent_path)?;
    dir.sync_all()?;

    Ok(())
}

pub fn raw_buf_to_u32(buffer: &[u8]) -> Result<u32, Box<dyn Error>> {
    let mut result: u32 = 0;
    let mut buf = buffer.iter();
    for _i in 0..4 {
        match buf.next() {
            Some(&byte) => {
                result = (result << 8) | byte as u32;
            }
            None => return Err("Given raw buffer is less than 4 byte".into()),
        }
    }
    Ok(result)
}

pub fn raw_buf_to_u16(buffer: &[u8]) -> Result<u16, Box<dyn Error>> {
    let mut result: u16 = 0;
    let mut buf = buffer.iter();
    for _i in 0..2 {
        match buf.next() {
            Some(&byte) => {
                result = (result << 8) | byte as u16;
            }
            None => {
                return Err("Given raw buffer is less than 2 byte".into());
            }
        }
    }

    Ok(result)
}

///
///Take relative or absolute directory path and file name to find.
///Return absolute Pathbuf of a file that match the given file name.
///
pub fn find_file_by_name<P: AsRef<Path>>(
    directory_path: P,
    file_name: &str,
) -> Result<Option<PathBuf>, Box<dyn Error>> {
    //get absolute path of directory if it exists.
    let mut patter_matched_files = Vec::new();
    let dir = get_absolute_path(directory_path)?;

    let paths = fs::read_dir(&dir);
    for path in paths? {
        let file_path = path?.file_name();
        let abs_file_path = dir.join(Path::new(&file_path));
        let is_file = abs_file_path.is_file();
        println!("{is_file}");

        if is_file {
            if let Some(file) = file_path.to_str() {
                if file.contains(file_name) {
                    patter_matched_files.push(abs_file_path);
                }
            }
        }
    }

    if patter_matched_files.len() == 0 {
        return Ok(None);
    }

    if patter_matched_files.len() > 1 {
        return Err("Found too many files with the given pattern".into());
    }

    Ok(Some(patter_matched_files[0].clone()))
}

pub fn convert_buff_to_integer(buffer: &[u8]) -> u32 {
    let mut result: u32 = 0;
    for &byte in buffer {
        result = (result << 8) | byte as u32;
    }
    result
}
const BUFFER_SIZE: usize = 512;
pub fn append_content_atomically(path: &Path, content: &[u8]) -> Result<(), Box<dyn Error>> {
    let file_path = get_absolute_path(path)?;
    let file_content = File::open(&file_path)?;
    let mut collected_buffer: Vec<u8> = Vec::new();
    let mut reader = BufReader::with_capacity(BUFFER_SIZE, file_content);
    loop {
        let buffer = reader.fill_buf()?;

        let buffer_length = buffer.len();

        // BufRead could not read any bytes.
        // The file must have completely been read.
        if buffer_length == 0 {
            break;
        }
        collected_buffer.extend_from_slice(buffer);

        // All bytes consumed from the buffer
        // should not be read again.
        reader.consume(buffer_length);
    }
    let parent = file_path.parent();
    let parent_path = match parent {
        Some(parent_path) => parent_path,
        None => return Err("Path has no parent directory".into()),
    };

    if !parent_path.try_exists()? {
        return Err("Parent directory doesn't exist".into());
    }

    let tmp = parent_path.join("tmp");

    let mut file = File::options().append(true).create(true).open(&tmp)?;

    file.write_all(&collected_buffer)?;
    file.write_all(content)?;
    file.sync_all()?;

    rename(tmp, &file_path)?;

    let dir = File::open(parent_path)?;
    dir.sync_all()?;

    Ok(())
}
