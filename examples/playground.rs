use chrono::{DateTime, Local, TimeZone};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use zgit::dto::index::IndexEntry;
use zgit::read_object;
use zgit::utlis::{build_tree, build_tree_structure, find_null, read_tree_recursive};
fn main() {
    let dummy_entries = vec![
        IndexEntry {
            ctime_secs: 1700000001,
            ctime_nsec: 0,
            mtime_secs: 1700000001,
            mtime_nsec: 0,
            dev: 2050,
            ino: 1001,
            mode: 0o100644,
            uid: 1000,
            gid: 1000,
            file_size: 120,
            oid: [1; 20],
            flags: 0,
            path: "README.md".to_string(),
        },
        IndexEntry {
            ctime_secs: 1700000002,
            ctime_nsec: 0,
            mtime_secs: 1700000002,
            mtime_nsec: 0,
            dev: 2050,
            ino: 1002,
            mode: 0o100644,
            uid: 1000,
            gid: 1000,
            file_size: 340,
            oid: [2; 20],
            flags: 0,
            path: "src/main.rs".to_string(),
        },
        IndexEntry {
            ctime_secs: 1700000003,
            ctime_nsec: 0,
            mtime_secs: 1700000003,
            mtime_nsec: 0,
            dev: 2050,
            ino: 1003,
            mode: 0o100644,
            uid: 1000,
            gid: 1000,
            file_size: 210,
            oid: [3; 20],
            flags: 0,
            path: "src/lib.rs".to_string(),
        },
        IndexEntry {
            ctime_secs: 1700000004,
            ctime_nsec: 0,
            mtime_secs: 1700000004,
            mtime_nsec: 0,
            dev: 2050,
            ino: 1004,
            mode: 0o100644,
            uid: 1000,
            gid: 1000,
            file_size: 180,
            oid: [4; 20],
            flags: 0,
            path: "src/utils/math.rs".to_string(),
        },
        IndexEntry {
            ctime_secs: 1700000005,
            ctime_nsec: 0,
            mtime_secs: 1700000005,
            mtime_nsec: 0,
            dev: 2050,
            ino: 1005,
            mode: 0o100644,
            uid: 1000,
            gid: 1000,
            file_size: 160,
            oid: [5; 20],
            flags: 0,
            path: "src/utils/io.rs".to_string(),
        },
        IndexEntry {
            ctime_secs: 1700000006,
            ctime_nsec: 0,
            mtime_secs: 1700000006,
            mtime_nsec: 0,
            dev: 2050,
            ino: 1006,
            mode: 0o100644,
            uid: 1000,
            gid: 1000,
            file_size: 90,
            oid: [6; 20],
            flags: 0,
            path: "tests/test_basic.rs".to_string(),
        },
    ];

    let some1 = build_tree_structure(dummy_entries);
    println!("{:?}", some1);
    let some = build_tree(&some1).unwrap();
    // println!("{:?}", some);

    let read_tree_recusive_res = read_tree_recursive(some, PathBuf::new());
    // println!("read_tree_recusive_res {read_tree_recusive_res:?}");
    // println!("oid prefix {:?}", hex::encode(some));
    // let some2 = read_object(Path::new("."), hex::encode(some).as_str());

    // println!("{:?}", some2);
}
