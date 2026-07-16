use std::{collections::BTreeMap, path::PathBuf};

use crate::Oid;

#[derive(Debug)]
pub enum Node {
    File { mode: u32, oid: Oid },
    Dir { children: BTreeMap<String, Node> },
}

pub struct Status {
    pub untracked: Vec<PathBuf>,
    pub modified: Vec<PathBuf>,
    pub staged: Vec<PathBuf>,
    pub deleted: Vec<PathBuf>,
}
