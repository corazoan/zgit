use chrono::{DateTime, Local};

use crate::dto::index::IndexEntry;
use crate::dto::tree::{self, Node, Status};
use crate::{Oid, read_index, read_object, store_object};
use std::collections::{BTreeMap, HashMap};
use std::fs::{File, rename};
use std::io::{self, BufRead, BufReader, Cursor};
use std::{
    error::Error,
    fs::{self},
    io::Write,
    path::{Path, PathBuf},
};
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

pub fn build_tree_structure(entries: Vec<IndexEntry>) -> Node {
    let mut root = Node::Dir {
        children: BTreeMap::new(),
    };

    for entry in entries {
        let parts: Vec<&str> = entry.path.split('/').collect();
        let mut current: &mut Node = &mut root;

        for (i, part) in parts.iter().enumerate() {
            let is_last = i == parts.len() - 1;

            // create a block to end the borrow each iteration
            current = match current {
                Node::Dir { children } => {
                    if is_last {
                        children.insert(
                            part.to_string(),
                            Node::File {
                                mode: entry.mode,
                                oid: entry.oid,
                            },
                        );
                        break;
                    } else {
                        children
                            .entry(part.to_string())
                            .or_insert_with(|| Node::Dir {
                                children: BTreeMap::new(),
                            })
                    }
                }
                Node::File { .. } => unreachable!(),
            };
        }
    }

    root
}

pub fn build_tree(node: &Node) -> Result<Oid, Box<dyn Error>> {
    let mut content = Vec::new();

    let children = match node {
        Node::Dir { children } => children,
        _ => return Err("build_tree called on non-directory node".into()),
    };

    for (name, child) in children.iter() {
        match child {
            Node::Dir { .. } => {
                let child_oid = build_tree(child)?;
                content.extend(b"40000 ");
                content.extend(name.as_bytes());
                content.push(0);
                content.extend(child_oid);
            }
            Node::File { mode, oid } => {
                content.extend(format!("{} {}\0", mode, name).as_bytes());
                content.extend(oid);
            }
        }
    }

    let mut source = Cursor::new(content);
    let tree_oid = store_object(Path::new("."), &ObjType::Tree, &mut source)?;
    Ok(tree_oid)
}

pub fn build_commit(
    tree_oid: Oid,
    parent_oid: Option<Oid>,
    author: &str,
    email: &str,
    message: &str,
) -> Result<Oid, Box<dyn Error>> {
    let mut commit = String::from("");

    // let tree_hex = ;
    commit.push_str(format!("tree {}\n", hex::encode(tree_oid)).as_str());
    if let Some(parent_oid) = parent_oid {
        commit.push_str(format!("parent {}\n", hex::encode(parent_oid)).as_str());
    }

    commit.push_str(
        format!(
            "Author: {} <{}> {} {}\n",
            author,
            email,
            Local::now().timestamp().to_string().as_str(),
            Local::now().offset().to_string().as_str(),
        )
        .as_str(),
    );
    commit.push_str(
        format!(
            "Commiter: {} <{}> {} {}\n",
            author,
            email,
            Local::now().timestamp().to_string().as_str(),
            Local::now().offset().to_string().as_str(),
        )
        .as_str(),
    );
    commit.push_str("\n");
    commit.push_str(message);
    commit.push_str("\n");
    store_object(Path::new("."), &ObjType::Commit, &mut Cursor::new(commit))
}
//
pub fn read_tree_recursive(
    tree_oid: Oid,
    base: PathBuf,
) -> Result<HashMap<PathBuf, Oid>, Box<dyn Error>> {
    let mut map = HashMap::new();

    let (_, data) = read_object(Path::new("."), hex::encode(tree_oid).as_str())?;

    let header_end = data
        .iter()
        .position(|&b| b == 0)
        .ok_or("Invalid tree object: missing header")?;

    // println!("from read{}", hex::encode(tree_oid));
    let mut i = header_end + 1;
    while i < data.len() {
        // println!("{:?}", data);
        let mode_end = find_spaces(&data, i)?;
        let mode = &data[i..mode_end];
        // println!("it must be expected {:?} {:?}", b"40000", mode);
        //parse

        let name_end = find_null(&data, mode_end + 1)?;
        let name = &data[mode_end + 1..name_end];
        let name = String::from_utf8(name.to_vec())?;
        // println!("{}", name);
        let oid_start = name_end + 1;
        let oid = to_array(&data[oid_start..oid_start + 20])?;

        let path = base.join(name);

        if mode == b"40000" {
            let subtree = read_tree_recursive(oid, path.clone())?;
            map.extend(subtree);
        } else {
            map.insert(path, oid);
        }
        i = oid_start + 20;
    }
    Ok(map)
}

fn to_array(slice: &[u8]) -> Result<[u8; 20], &'static str> {
    slice.try_into().map_err(|_| "slice length != 20")
}

pub fn find_spaces(data: &Vec<u8>, index: usize) -> Result<usize, Box<dyn Error>> {
    for (i, element) in data.iter().enumerate() {
        if element == &b' ' && i >= index {
            return Ok(i);
        }
    }
    return Err("No space found in given data".into());
}
pub fn find_null(data: &Vec<u8>, index: usize) -> Result<usize, Box<dyn Error>> {
    for (i, element) in data.iter().enumerate() {
        if element == &b'\0' && i >= index {
            return Ok(i);
        }
    }
    return Err("No null found in given data".into());
}

// pub fn get_status(repo: &Path) -> Result<Status, Box<dyn Error>> {
//     let index = read_index(repo)?;

// }

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
