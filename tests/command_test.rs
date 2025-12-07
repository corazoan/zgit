// use std::path::Path;

use std::{
    io::Cursor,
    path::{Path, PathBuf},
};
use zgit::{compute_oid, find_repo, format_object_content, store_object};
use zgit::{read_object, utlis::ObjType};

#[test]
fn test_compute_oid() {
    use std::io::Cursor;
    let mut data = Cursor::new("hello");
    let oid = compute_oid(&ObjType::Blob, &mut data).unwrap();
    assert_eq!(hex::encode(oid), "b6fc4c620b67d95f953a5c1c1230aaab5db5a1b0")
}

#[test]
fn test_format_object_content() {
    let data = Cursor::new("hello");
    let some = format_object_content(&ObjType::Blob, data).unwrap();
    assert_eq!(some, b"blob 5\0hello")
}

//Check find_repo function if path given path is relative and a valid zgit repo
#[test]
fn test_find_repo() {
    let path = find_repo(Some(Path::new("../test").to_path_buf()), None).unwrap();
    assert_eq!(
        path,
        Some(PathBuf::from("/home/lightboy/code/learn_rust/test"))
    )
}

//Check find_repo function if path given path is absolute and a invalid zgit repo
#[test]
fn find_repo_abs_invalid_zgit_repo_path() {
    let path = find_repo(Some(Path::new("/home/lightboy").to_path_buf()), Some(false)).unwrap();
    assert_eq!(path, None)
}

#[test]
fn test_store_object() {
    let mut source = Cursor::new("Implementing version control system");
    let repo = Path::new("../test");
    let oid = store_object(repo, &ObjType::Commit, &mut source).unwrap();

    let hash = hex::encode(oid);
    let dir_name = &hash[0..2];
    let file_name = &hash[2..];

    assert_eq!(dir_name, "68");
    assert_eq!(file_name, "63dbb7f3f6c7936432142d546034738fcdfdd7");

    let path_to_file = repo.join(".zgit/objects").join(dir_name).join(file_name);
    assert_eq!(path_to_file.try_exists().unwrap(), true);
    let (obj_type, file_content) = read_object(repo, &hash).unwrap();

    assert_eq!(obj_type, ObjType::Commit);
    assert_eq!(
        String::from_utf8(file_content).unwrap(),
        "commit 35\0Implementing version control system"
    );
}
#[test]
fn some_read_object() {}
