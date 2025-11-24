use flate2::{Compression, write::ZlibEncoder};
use sha1::{Digest, Sha1};
use std::{
    error::Error,
    fs::{self, File, create_dir_all, rename},
    io::{self, ErrorKind, Read, Write},
    path::{Path, PathBuf},
    u8,
};

fn get_absolute_path<P: AsRef<Path>>(path: P) -> io::Result<PathBuf> {
    let input_path = path.as_ref();

    // canonicalize converts relative path → absolute path
    // and also resolves symbolic links, `.` and `..`
    let abs_path = input_path.canonicalize()?;

    Ok(abs_path)
}
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

///
pub fn write_content_atomically(path: &Path, content: &[u8]) -> Result<(), Box<dyn Error>> {
    get_absolute_path(path);
    let parent = path.parent();
    let parent_path = match parent {
        Some(parent_path) => parent_path,
        None => {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Path has no parent directory",
            )) as Box<dyn std::error::Error>);
        }
    };

    let is_parent_dir_exist = parent_path.try_exists()?;

    if !is_parent_dir_exist {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Parent directory doesn't exist",
        )) as Box<dyn std::error::Error>);
    }

    let tmp = parent_path.join("tmp");

    let mut file = File::create(&tmp)?;
    file.write_all(content)?;
    file.sync_all()?;

    rename(tmp, path)?;

    let dir = File::open(parent_path)?;
    dir.sync_all()?;

    Ok(())
}

type Oid = [u8; 20];

///Return blob of hash. Hash generated using header byte + data buffer
/// ```
/// use std::io::Cursor;
///let mut data = Cursor::new("hello")
/// let oid = compute_oid("blob", &mut data).unwrap();
/// assert_eq!(hex::encode(oid), "b6fc4c620b67d95f953a5c1c1230aaab5db5a1b0")
///
/// ```
pub fn compute_oid(tp: &str, data: &mut impl Read) -> Result<Oid, Box<dyn Error>> {
    let mut header = String::new();

    match tp {
        "blob" => header.push_str(tp),
        "tree" => header.push_str(tp),
        "commit" => header.push_str(tp),
        "tag" => header.push_str(tp),
        _ => {
            return Err("Unknown Object type".into());
        }
    };
    let mut buffer = Vec::new();
    let size = data.read_to_end(&mut buffer)?;
    header.push_str(format!(" {}\0", size).as_str());
    let mut hasher = Sha1::new();

    hasher.update(header.as_bytes());
    hasher.update(buffer);
    let result = hasher.finalize();

    Ok(result.into())
}

pub fn format_object_content(tp: &str, mut data: impl Read) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut header = String::new();

    match tp {
        "blob" => header.push_str(tp),
        "tree" => header.push_str(tp),
        "commit" => header.push_str(tp),
        "tag" => header.push_str(tp),
        _ => return Err("".into()),
    };
    let mut buffer = Vec::new();
    let size = data.read_to_end(&mut buffer)?;
    header.push_str(format!(" {}\0", size).as_str());
    let mut concatened = header.as_bytes().to_vec();
    concatened.extend(buffer);
    Ok(concatened)
}

pub enum ObjType {
    Blob,
    Commit,
    Tag,
}

pub fn store_object(
    repo: &Path,
    obj_type: ObjType,
    source: &mut impl Read,
) -> Result<Oid, Box<dyn Error>> {
    let tp = match obj_type {
        ObjType::Blob => "blob",
        ObjType::Commit => "commit",
        ObjType::Tag => "tag",
    };

    let oid = compute_oid(&tp, source)?;

    let hash = hex::encode(oid);
    let dir = &hash[0..3];
    let file = &hash[3..];
    //If path to file that we wanna make already exists then early return oid
    // Otherwise create directory
    if let Some(path) = find_repo(Some(repo.to_path_buf()), Some(true))? {
        let path_to_make = path.join("./.zgit/objects").join(dir).join(file);

        if path_to_make.try_exists()? {
            return Ok(oid);
        }

        let dir = path_to_make.parent();
        match dir {
            Some(path) => create_dir_all(path)?,
            None => return Err("Unexpected error occure".into()),
        };
    }

    let stream = format_object_content(tp, source)?;

    let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
    e.write_all(&stream)?;
    let compressed_bytes = e.finish()?;

    // let path = write_content_atomically(repo.join("./.zgit/objects"));
    // join(dir).join(file),
    // &compressed_bytes,

    Ok(oid)
}
