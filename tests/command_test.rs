// use std::path::Path;

use std::{
    io::Write,
    path::{Path, PathBuf},
};

use zgit::{compute_oid, find_repo, format_object_content};

// #[test]
// fn test_find_repo() -> Result<(), Box<dyn std::error::Error>> {
//     debug_assert_eq!(
//         zgit::find_repo(Some(Path::new("/home/lightboy").to_path_buf()), Some(false)),
//         Ok(None)
//     )
// }
#[test]
fn test_compute_oid() {
    use std::io::Cursor;
    let mut data = Cursor::new("hello");
    let oid = compute_oid("blob", &mut data).unwrap();
    assert_eq!(hex::encode(oid), "b6fc4c620b67d95f953a5c1c1230aaab5db5a1b0")
}

#[test]
fn test_format_object_content() {
    let some = format_object_content("blob", b"hello".as_ref()).unwrap();
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
