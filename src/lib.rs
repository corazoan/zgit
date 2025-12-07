use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use sha1::{Digest, Sha1};
use std::io::{BufRead, BufReader};
use std::{
    error::Error,
    fs::{self, File, create_dir_all},
    io::{self, ErrorKind, Read, Seek, Write},
    path::{Path, PathBuf},
    u8,
};

use crate::utlis::{ObjType, find_file_by_name, get_absolute_path, write_content_atomically};

pub mod utlis;
///Give error when required argument is Option<true> and provide path is not a git repository.
///otherwise it return Option<pathbuf>
pub fn find_repo(
    path: Option<PathBuf>,
    required: Option<bool>,
) -> Result<Option<PathBuf>, std::io::Error> {
    let required = required.unwrap_or(true);
    let path = path.unwrap_or(Path::new(".").to_path_buf());
    let abs_path = get_absolute_path(path)?;

    let path = fs::canonicalize(abs_path)?;

    if path.join(".zgit").is_dir() {
        return Ok(Some(path));
    }

    let parent = fs::canonicalize(path.join(".."))?;
    if parent == path {
        if required {
            return Err(io::Error::new(
                ErrorKind::NotADirectory,
                "Not a git directory",
            ));
        }
        return Ok(None);
    };
    return find_repo(Some(parent), Some(required));
}

pub fn init_zgit_repo() -> Result<(), Box<dyn Error>> {
    //Early return if already a zgit repository.
    if let Ok(path) = find_repo(Some(Path::new(".").to_path_buf()), Some(false)) {
        if let Some(path) = path {
            println!("\x1b[92mAlready a zgit repository in {:?} \x1b[00m", path);
            return Ok(());
        }
    }

    fs::create_dir_all(".zgit")?;
    //all refs directory and sub directory
    fs::create_dir_all(".zgit/refs")?;
    fs::create_dir_all(".zgit/refs/heads")?;
    fs::create_dir_all(".zgit/refs/tags")?;
    //all objects directory and sub directory
    fs::create_dir_all(".zgit/objects")?;
    fs::create_dir_all(".zgit/objects/info")?;
    fs::create_dir_all(".zgit/objects/pack")?;
    //other directory
    fs::create_dir_all(".zgit/hooks")?;
    fs::create_dir_all(".zgit/info")?;

    write_content_atomically(Path::new(".zgit/HEAD"), b"ref: refs/heads/main\n")?;
    println!("\x1b[92mSuccessfully initialize zgit repository \x1b[00m");
    Ok(())
}

type Oid = [u8; 20];
const BUFFER_SIZE: usize = 512;
///Return blob of hash. Hash generated using header byte + data buffer
/// ```
/// use std::io::Cursor;
/// use zgit::compute_oid;
/// use zgit::utlis::ObjType;
///let mut data = Cursor::new("hello");
/// let oid = compute_oid(&ObjType::Blob, &mut data).unwrap();
/// assert_eq!(hex::encode(oid), "b6fc4c620b67d95f953a5c1c1230aaab5db5a1b0")
///
/// ```
pub fn compute_oid(tp: &ObjType, data: &mut (impl Read + Seek)) -> Result<Oid, Box<dyn Error>> {
    let mut header = String::new();

    match tp {
        ObjType::Commit => header.push_str("commit"),
        ObjType::Tree => header.push_str("tree"),
        ObjType::Blob => header.push_str("blob"),
        ObjType::Tag => header.push_str("tag"),
    };
    let mut collected_buffer: Vec<u8> = Vec::new();
    let mut reader = BufReader::with_capacity(BUFFER_SIZE, &mut *data);
    let mut size = 0;
    loop {
        let buffer = reader.fill_buf()?;

        let buffer_length = buffer.len();

        // BufRead could not read any bytes.
        // The file must have completely been read.
        if buffer_length == 0 {
            break;
        }
        size += buffer_length;
        collected_buffer.extend_from_slice(buffer);

        // All bytes consumed from the buffer
        // should not be read again.
        reader.consume(buffer_length);
    }
    data.rewind()?;
    header.push_str(format!(" {}\0", size).as_str());
    let mut hasher = Sha1::new();

    hasher.update(header.as_bytes());
    hasher.update(&collected_buffer);
    let result = hasher.finalize();

    Ok(result.into())
}

pub fn format_object_content(
    tp: &ObjType,
    data: &mut (impl Read + Seek),
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut header = String::new();

    match tp {
        ObjType::Commit => header.push_str("commit"),
        ObjType::Tree => header.push_str("tree"),
        ObjType::Blob => header.push_str("blob"),
        ObjType::Tag => header.push_str("tag"),
    };
    let mut collected_buffer: Vec<u8> = Vec::new();
    let mut reader = BufReader::with_capacity(BUFFER_SIZE, &mut *data);
    let mut size = 0;
    loop {
        let buffer = reader.fill_buf()?;

        let buffer_length = buffer.len();

        // BufRead could not read any bytes.
        // The file must have completely been read.
        if buffer_length == 0 {
            break;
        }
        size += buffer_length;
        collected_buffer.extend_from_slice(buffer);

        // All bytes consumed from the buffer
        // should not be read again.
        reader.consume(buffer_length);
    }
    data.rewind()?;

    header.push_str(format!(" {}\0", size).as_str());
    let mut concatened = header.as_bytes().to_vec();
    concatened.extend(collected_buffer);
    Ok(concatened)
}

pub fn store_object(
    repo: &Path,
    obj_type: &ObjType,
    source: &mut (impl Read + Seek),
) -> Result<Oid, Box<dyn Error>> {
    if let Some(path) = find_repo(Some(repo.to_path_buf()), Some(true))? {
        let oid = compute_oid(&obj_type, source)?;

        let hash = hex::encode(oid);
        let dir = &hash[0..2];
        let file = &hash[2..];
        //If path to file that we wanna make already exists then early return oid
        // Otherwise create directory
        let path_to_make = path.join(".zgit/objects").join(dir).join(file);
        if path_to_make.try_exists()? {
            return Ok(oid);
        }

        let dir = path_to_make.parent();
        let dir = match dir {
            Some(path) => path,
            None => return Err("Unexpected error occure".into()),
        };
        create_dir_all(dir)?;
        File::create(&path_to_make)?;
        let stream = format_object_content(&obj_type, source)?;
        let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
        e.write_all(&stream)?;
        let compressed_bytes = e.finish()?;
        write_content_atomically(&path_to_make, &compressed_bytes)?;
        return Ok(oid);
    }
    return Err("Not a git repository (or any parent up to mount point /".into());
}

pub fn read_object(repo: &Path, oid_or_prefix: &str) -> Result<(ObjType, Vec<u8>), Box<dyn Error>> {
    if let Some(root_dir_of_repo) = find_repo(Some(repo.to_path_buf()), Some(true))? {
        if oid_or_prefix.len() < 2 {
            return Err("Provided prefix is too short.".into());
        }

        let dir = &oid_or_prefix[0..2];
        let file_name = &oid_or_prefix[2..];
        let dir = root_dir_of_repo.join(".zgit/objects").join(dir);
        if !dir.try_exists()? {
            return Err(format!("Object not found with given prefix {}", oid_or_prefix).into());
        }
        let matched_file = find_file_by_name(dir, file_name)?;

        match matched_file {
            None => return Err("File not found with give oid prefix".into()),
            Some(file_path) => {
                let compressed_data = File::open(file_path)?;
                let mut d = ZlibDecoder::new(compressed_data);
                let mut s = String::new();
                d.read_to_string(&mut s)?;

                let mut obj_type = s.split(" ");
                let obj_type = obj_type.next();
                let obj_type = match obj_type {
                    None => return Err("Invalid hash object".into()),
                    Some(tp) => tp,
                };

                let obj_type = match obj_type {
                    "commit" => ObjType::Commit,
                    "blob" => ObjType::Blob,
                    "tag" => ObjType::Tag,
                    "tree" => ObjType::Tree,
                    _ => return Err("Received invalid object type".into()),
                };

                return Ok((obj_type, s.as_bytes().into()));
            }
        }
    }
    return Err(format!("Object not found with given prefix {}", oid_or_prefix).into());
}
