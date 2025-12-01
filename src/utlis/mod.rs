use std::{
    error::Error,
    fs::{self, File, rename},
    io::{self, Write},
    path::{Path, PathBuf},
};

#[derive(Debug, PartialEq)]
pub enum ObjType {
    Blob,
    Commit,
    Tag,
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
