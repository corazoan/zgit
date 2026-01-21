use std::io::Cursor;
use std::path::{Path, PathBuf};
use zgit::utlis::ObjType;
use zgit::{compute_oid, find_repo, format_object_content};

#[test]
fn test_compute_oid() {
    use std::io::Cursor;
    let mut data = Cursor::new("hello");
    let oid = compute_oid(&ObjType::Blob, &mut data).unwrap();
    assert_eq!(hex::encode(oid), "b6fc4c620b67d95f953a5c1c1230aaab5db5a1b0")
}

#[test]
fn test_format_object_content() {
    let mut data = Cursor::new("hello");
    let some = format_object_content(&ObjType::Blob, &mut data).unwrap();
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
