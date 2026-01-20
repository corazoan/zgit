use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use sha1::{Digest, Sha1};
use std::io::{BufRead, BufReader};
use std::vec;
use std::{
    error::Error,
    fs::{self, File, create_dir_all},
    io::{self, ErrorKind, Read, Seek, Write},
    path::{Path, PathBuf},
    u8,
};

use crate::utlis::{
    ObjType, append_content_atomically, find_file_by_name, get_absolute_path, raw_buf_to_u16,
    raw_buf_to_u32, write_content_atomically,
};

pub mod utlis;
/// Locate the nearest Git repository by walking upward from `path`.
///
/// This function checks whether the given `path` (or the current directory if
/// `None`) is inside a Git repository by looking for a `.git` directory. If the
/// directory is not a repository, it moves to the parent directory and repeats
/// until it reaches the filesystem root.
///
/// Behaviour notes:
/// - If `path` is `None`, the search starts at the current directory (`"."`).
/// - If `required` is `None`, it defaults to `true`.
/// - The function canonicalizes the provided path (resolving symlinks) and
///   returns an **absolute** `PathBuf` if a repository is found.
/// - The function is recursive: it climbs parent directories until it finds a
///   `.git` directory or reaches the filesystem root.
///
/// # Arguments
///
/// * `path` - Optional starting path for the search. When `None` the search
///   begins at the current working directory.
/// * `required` - Optional flag controlling error behaviour:
///   * `Some(true)` or `None` (default) — return `Err` when no repository is
///     found after reaching the root.
///   * `Some(false)` — return `Ok(None)` when no repository is found.
///
/// # Returns
///
/// * `Ok(Some(PathBuf))` — absolute path to the directory containing `.git`.
/// * `Ok(None)` — no Git repository found (only when `required` is `Some(false)`).
/// * `Err(std::io::Error)` — an I/O error occurred (or `required` was true and
///   no repository was found). In the latter case the function returns an
///   `io::Error` constructed with `ErrorKind::NotADirectory` and the message
///   `"Not a git directory"`.
///
/// # Errors
///
/// Returns any I/O errors produced while canonicalizing paths (for example,
/// permission errors) or the explicit `NotADirectory` error when `required` is
/// true and no `.git` directory exists in any ancestor.
///
/// # Examples
///
/// ```rust
/// # use std::path::PathBuf;
/// # fn example() -> Result<(), std::io::Error> {
/// // Search from the current directory and treat "not found" as an error:
/// match find_repo(None, None)? {
///     Some(repo_root) => println!("Found repo at {}", repo_root.display()),
///     None => println!("No repository found (this line is reachable only if required=false)"),
/// }
///
/// // Start from a specific path and allow "not found":
/// let start = Some(PathBuf::from("/some/sub/dir"));
/// match find_repo(start, Some(false))? {
///     Some(repo_root) => println!("Repo root: {}", repo_root.display()),
///     None => println!("No Git repository in any ancestor of /some/sub/dir"),
/// }
/// # Ok(()) }
/// # let _ = example();
/// ```
///
/// # Panics
///
/// This function does not intentionally panic. Any unexpected panics would be
/// caused by underlying platform or library bugs.
///
/// # Implementation detail
///
/// The function relies on `fs::canonicalize` to produce absolute, normalized
/// paths and then checks for a `.git` directory in the canonicalized path and
/// its ancestors.
pub fn find_repo(
    path: Option<PathBuf>,
    required: Option<bool>,
) -> Result<Option<PathBuf>, std::io::Error> {
    let required = required.unwrap_or(true);
    let path = path.unwrap_or(Path::new(".").to_path_buf());
    let abs_path = get_absolute_path(path)?;

    let path = fs::canonicalize(abs_path)?;

    if path.join(".git").is_dir() {
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

pub fn write_index<P: AsRef<Path>>(path: P, index: &mut Index) -> Result<(), Box<dyn Error>> {
    match find_repo(Some(path.as_ref().to_path_buf()), None)? {
        None => Err("Given Repo is not a git repo".into()),
        Some(path) => {
            let index_file_path = path.join(".zgit/index");

            if fs::metadata(&index_file_path)
                .map(|m| m.is_file())
                .unwrap_or(false)
            {
                let existing_entry = read_index(path.as_ref())?;
                let filtered: Vec<IndexEntry> = existing_entry
                    .entries
                    .into_iter()
                    .filter(|old| !index.entries.iter().any(|new| new.path == old.path))
                    .collect();
                index.entries.extend(filtered);
            }
            File::create(&index_file_path)?;

            let mut hasher = Sha1::new();

            append_content_atomically(&index_file_path, b"DIRC")?;
            hasher.update(b"DIRC");

            let version: u32 = 2;
            let no_of_entry: u32 = index.entries.len().try_into()?;
            append_content_atomically(&index_file_path, &version.to_be_bytes())?;
            hasher.update(&version.to_be_bytes());

            append_content_atomically(&index_file_path, &no_of_entry.to_be_bytes())?;
            hasher.update(&no_of_entry.to_be_bytes());
            index.sort();
            index.write_to_file(&index_file_path)?;

            for entry in &index.entries {
                hasher.update(entry.ctime_secs.to_be_bytes());
                hasher.update(entry.ctime_nsec.to_be_bytes());
                hasher.update(entry.mtime_secs.to_be_bytes());
                hasher.update(entry.mtime_nsec.to_be_bytes());
                hasher.update(entry.dev.to_be_bytes());
                hasher.update(entry.ino.to_be_bytes());
                hasher.update(entry.mode.to_be_bytes());
                hasher.update(entry.uid.to_be_bytes());
                hasher.update(entry.gid.to_be_bytes());
                hasher.update(entry.file_size.to_be_bytes());
                hasher.update(entry.oid);
                hasher.update(entry.flags.to_be_bytes());
                hasher.update(entry.path.as_bytes());
                hasher.update(&[0]);
                let entry_len = 62 + entry.path.as_bytes().len() + 1;
                let pad_len = (8 - (entry_len % 8)) % 8;
                let pad = vec![b'\0'; pad_len];
                hasher.update(pad);
            }
            let result: Oid = hasher.finalize().into();
            println!(" result {:?}", result);
            for i in result {
                append_content_atomically(&index_file_path, &[i])?;
            }

            Ok(())
        }
    }
}

pub fn read_index(path: &Path) -> Result<Index, Box<dyn Error>> {
    let mut index = Index {
        entries: Vec::new(),
    };
    if let Some(path) = find_repo(Some(path.to_path_buf()), None)? {
        let index_file = path.join(".zgit/index");
        let file = File::open(index_file)?;
        let mut collect_buffer: Vec<u8> = Vec::new();
        let mut reader = BufReader::with_capacity(12, file);

        loop {
            let buffer = reader.fill_buf()?;
            let buffer_length = buffer.len();
            if buffer_length == 0 {
                break;
            }

            collect_buffer.extend_from_slice(buffer);
            reader.consume(buffer_length);
        }

        let header_part_byte = vec![4, 4, 4];

        if collect_buffer.len() < 12 {
            return Err("Index file is too small to read".into());
        }
        let mut start_index = 0;
        let mut num_entries = 0;
        for i in header_part_byte {
            let buffer = &collect_buffer.get(start_index..start_index + i);

            if start_index == 0 {
                match buffer {
                    None => return Err("Unexpected end of line".into()),
                    Some(buf) => {
                        let signature = String::from_utf8_lossy(*buf);
                        if signature != "DIRC" {
                            return Err("Invalid Signature".into());
                        }
                    }
                }
            }
            if start_index == 4 {
                match buffer {
                    None => return Err("Unexpected end of line".into()),
                    Some(buf) => {
                        let version = raw_buf_to_u32(*buf)?;
                        if version != 2 {
                            return Err("Invalid Version".into());
                        }
                    }
                }
            }
            if start_index == 8 {
                match buffer {
                    None => return Err("Unexpected end of line".into()),
                    Some(buf) => {
                        num_entries = raw_buf_to_u32(*buf)?;
                    }
                }
            }
            start_index += i;
        }
        let min_expect_file_size = 12 + 62 * num_entries + 20;

        if collect_buffer.len() < min_expect_file_size as usize {
            return Err("Invalid file size".into());
        }

        let entry_sequence_byte = [4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 20, 2];
        for (i, _entry) in (0..num_entries).enumerate() {
            let mut ctime_secs: u32 = 0;
            let mut ctime_nsec: u32 = 0;
            let mut mtime_secs: u32 = 0;
            let mut mtime_nsec: u32 = 0;
            let mut dev: u32 = 0;
            let mut ino: u32 = 0;
            let mut mode: u32 = 0;
            let mut uid: u32 = 0;
            let mut gid: u32 = 0;
            let mut file_size: u32 = 0;
            let mut oid: Oid = [
                182, 252, 76, 98, 11, 103, 217, 95, 149, 58, 92, 28, 18, 48, 170, 171, 93, 181,
                161, 176,
            ];
            let mut flags: u16 = 0;
            let mut path: String = String::from("default_path");
            for (index, j) in entry_sequence_byte.iter().enumerate() {
                match index {
                    0 => {
                        let buffer = &collect_buffer.get(start_index..start_index + j);
                        match buffer {
                            None => return Err("Unexpected end of line".into()),
                            Some(buf) => {
                                ctime_secs = raw_buf_to_u32(*buf)?;
                            }
                        }
                    }
                    1 => {
                        let buffer = &collect_buffer.get(start_index..start_index + j);

                        match buffer {
                            None => return Err("Unexpected end of line".into()),
                            Some(buf) => {
                                ctime_nsec = raw_buf_to_u32(*buf)?;
                            }
                        }
                    }
                    2 => {
                        let buffer = &collect_buffer.get(start_index..start_index + j);
                        match buffer {
                            None => return Err("Unexpected end of line".into()),
                            Some(buf) => {
                                mtime_secs = raw_buf_to_u32(*buf)?;
                            }
                        }
                    }
                    3 => {
                        let buffer = &collect_buffer.get(start_index..start_index + j);
                        match buffer {
                            None => return Err("Unexpected end of line".into()),
                            Some(buf) => {
                                mtime_nsec = raw_buf_to_u32(*buf)?;
                            }
                        }
                    }
                    4 => {
                        let buffer = &collect_buffer.get(start_index..start_index + j);
                        match buffer {
                            None => return Err("Unexpected end of line".into()),
                            Some(buf) => {
                                dev = raw_buf_to_u32(*buf)?;
                            }
                        }
                    }
                    5 => {
                        let buffer = &collect_buffer.get(start_index..start_index + j);
                        match buffer {
                            None => return Err("Unexpected end of line".into()),
                            Some(buf) => {
                                ino = raw_buf_to_u32(*buf)?;
                            }
                        }
                    }
                    6 => {
                        let buffer = &collect_buffer.get(start_index..start_index + j);
                        match buffer {
                            None => return Err("Unexpected end of line".into()),
                            Some(buf) => {
                                mode = raw_buf_to_u32(*buf)?;
                            }
                        }
                    }
                    7 => {
                        let buffer = &collect_buffer.get(start_index..start_index + j);
                        match buffer {
                            None => return Err("Unexpected end of line".into()),
                            Some(buf) => {
                                uid = raw_buf_to_u32(*buf)?;
                            }
                        }
                    }
                    8 => {
                        let buffer = &collect_buffer.get(start_index..start_index + j);
                        match buffer {
                            None => return Err("Unexpected end of line".into()),
                            Some(buf) => {
                                gid = raw_buf_to_u32(*buf)?;
                            }
                        }
                    }
                    9 => {
                        let buffer = &collect_buffer.get(start_index..start_index + j);

                        match buffer {
                            None => return Err("Unexpected end of line".into()),
                            Some(buf) => {
                                file_size = raw_buf_to_u32(*buf)?;
                            }
                        }
                    }
                    10 => {
                        let buffer = &collect_buffer.get(start_index..start_index + j);
                        match buffer {
                            None => return Err("Unexpected end of line".into()),
                            Some(buf) => {
                                let some = *buf;
                                let arr: [u8; 20] =
                                    some.try_into().expect("slice must be 20 bytes");
                                oid = arr
                            }
                        }
                    }
                    11 => {
                        let buffer = &collect_buffer.get(start_index..start_index + j);
                        match buffer {
                            None => return Err("Unexpected end of line".into()),
                            Some(buf) => {
                                flags = raw_buf_to_u16(*buf)?;
                            }
                        }
                    }
                    _ => {
                        break;
                    }
                }
                start_index += j;
            }
            let mut path_buff = Vec::new();
            while let Some(&buf) = &collect_buffer.get(start_index) {
                if buf != b'\0' {
                    path_buff.push(buf);
                } else {
                    start_index += 1;

                    break;
                }
                start_index += 1;
            }
            let path_string = String::from_utf8(path_buff)?;
            let path_len = path_string.clone().into_bytes().len();
            path = path_string;
            index.entries.push(IndexEntry {
                ctime_secs,
                ctime_nsec,
                mtime_secs,
                mtime_nsec,
                dev,
                ino,
                mode,
                uid,
                gid,
                file_size,
                oid,
                flags,
                path,
            });

            let entry_len = 62 + path_len + 1;
            let pad_len = (8 - (entry_len % 8)) % 8;
            start_index += pad_len;
        }

        //now checking for checksum buffer
        if let Some(received_checksum) = &collect_buffer.get(start_index..start_index + 20) {
            let mut hasher = Sha1::new();
            match collect_buffer.get(0..collect_buffer.len() - 20) {
                None => return Err("can't able to get buffer from starting to end".into()),
                Some(buf) => hasher.update(buf),
            }

            let expect_checksum: Oid = hasher.finalize().into();

            if expect_checksum != *received_checksum {
                println!("checksum not equal");
                return Err("Invalid checksum".into());
            }
            println!("received checksum {:?}", received_checksum)
        }
    }

    Ok(index)
}
