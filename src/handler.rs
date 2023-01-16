use std::{collections::HashMap, rc::Rc};

use fuse::FileType;
use std::fmt::Debug;

use crate::identity::ItemIdentity;

#[derive(Debug, Clone)]
pub enum PathHandler<'a> {
    File(File<'a>),
    Directory(DirectoryListing<'a>),
}

impl<'a> PathHandler<'a> {
    pub fn get_type(&self) -> FileType {
        match self {
            PathHandler::File(_) => FileType::RegularFile,
            PathHandler::Directory(_) => FileType::Directory,
        }
    }
}

pub(crate) trait Identifiable {
    fn get_identity(&self) -> ItemIdentity;
}

impl<'a> Identifiable for PathHandler<'a> {
    fn get_identity(&self) -> ItemIdentity {
        match self {
            PathHandler::File(handler) => handler.get_identity(),
            PathHandler::Directory(handler) => handler.get_identity(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct File<'a> {
    identity: ItemIdentity,
    implementation: Rc<Box<dyn FileHandler + 'a>>,
}
impl<'a> File<'a> {
    pub fn from_impl(implementation: impl FileHandler + 'a) -> File<'a> {
        File {
            identity: ItemIdentity::new(),
            implementation: Rc::new(Box::new(implementation)),
        }
    }

    pub fn get_size(&self) -> u64 {
        self.implementation.get_size()
    }
}

impl<'a> Identifiable for File<'a> {
    fn get_identity(&self) -> ItemIdentity {
        self.identity
    }
}


#[derive(Debug, Clone)]
pub struct DirectoryListing<'a> {
    identity: ItemIdentity,
    items: HashMap<String, PathHandler<'a>>,
}
impl<'a> DirectoryListing<'a> {
    pub fn new() -> DirectoryListing<'a> {
        DirectoryListing {
            items: HashMap::new(),
            identity: ItemIdentity::new(),
        }
    }

    pub fn listdir(&self) -> Vec<(String, PathHandler)> {
        self.items.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }

    pub fn add_file(mut self, name: &str, file: impl FileHandler + 'a) -> Self {
        self.items.insert(name.to_string(), PathHandler::File(File::from_impl(file)));
        self
    }
}

impl<'a> Identifiable for DirectoryListing<'a> {
    fn get_identity(&self) -> ItemIdentity {
        self.identity
    }
}

pub trait FileHandler: std::fmt::Debug {
    fn get_size(&self) -> u64;
}