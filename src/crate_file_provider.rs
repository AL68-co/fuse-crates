use std::{
    collections::HashMap,
    ffi::OsString,
    io::Read,
    path::{Component, Path, PathBuf},
};

use crate::{DirChild, FuseFsImp};

#[derive(Debug)]
pub struct Tree {
    root: Directory,
}

impl Tree {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Tree {
        Tree {
            root: Directory {
                name: String::from("/"),
                children: None,
            },
        }
    }

    pub fn get(&self, path: &Path) -> Option<DirectoryChild> {
        if path == Path::new("/") {
            return Some(DirectoryChild::Directory(self.root.clone()));
        }
        if !path.starts_with(Component::RootDir) {
            panic!("Path must be absolute, but received {:?}", path);
        }
        let mut current_node = DirectoryChild::Directory(self.root.clone());
        for component in path.components().skip(1) {
            let component = component.as_os_str();
            match current_node {
                DirectoryChild::Directory(ref dir) => {
                    if dir.children.is_some() {
                        let children = dir.children.as_ref().expect("Checked above");
                        if children.contains_key(component) {
                            current_node = children.get(component).expect("Verified above").clone();
                        } else {
                            return None;
                        }
                    } else {
                        panic!("Tree not populated")
                    }
                }
                DirectoryChild::File(_) => {
                    panic!("File found where directory expected")
                }
            }
        }
        Some(current_node)
    }

    pub fn fill_tree<R: Read>(&mut self, arc: &mut tar::Archive<R>) {
        for (entry_index, entry) in arc.entries().unwrap().enumerate() {
            let entry = entry.unwrap();
            let path = entry.path().unwrap();
            let name = path.file_name().unwrap().to_str().unwrap();
            let mut path = path.to_path_buf();
            path.pop();
            let mut current_node = &mut self.root;
            for component in path.components() {
                let component = component.as_os_str();
                if current_node.children.is_some() {
                    let children = current_node.children.as_mut().expect("Checked above");
                    if children.contains_key(component) {
                        match children.get_mut(component).expect("Verified above") {
                            DirectoryChild::Directory(dir) => current_node = dir,
                            DirectoryChild::File(_) => {
                                panic!("File found where directory expected")
                            }
                        }
                    } else {
                        let dir = Directory {
                            name: component.to_str().unwrap().to_string(),
                            children: None,
                        };
                        children.insert(component.into(), DirectoryChild::Directory(dir));
                        current_node = children.get_mut(component).unwrap().as_directory_mut();
                    }
                } else {
                    let dir = Directory {
                        name: component.to_str().unwrap().to_string(),
                        children: None,
                    };
                    let mut children = HashMap::new();
                    children.insert(OsString::from(component), DirectoryChild::Directory(dir));
                    current_node.children = Some(children);
                    current_node = current_node
                        .children
                        .as_mut()
                        .unwrap()
                        .get_mut(component)
                        .unwrap()
                        .as_directory_mut();
                }
            }
            let file = File {
                name: String::from(name),
                index: entry_index,
                size: entry.size(),
            };
            if let Some(children) = &mut current_node.children {
                children.insert(OsString::from(name), DirectoryChild::File(file));
            } else {
                let mut children = HashMap::new();
                children.insert(OsString::from(name), DirectoryChild::File(file));
                current_node.children = Some(children);
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Directory {
    name: String,
    children: Option<HashMap<OsString, DirectoryChild>>,
}

impl Directory {
    fn into_iter(self) -> std::collections::hash_map::IntoValues<OsString, DirectoryChild> {
        self.children.unwrap().into_values()
    }
}

#[derive(Clone, Debug)]
pub enum DirectoryChild {
    Directory(Directory),
    File(File),
}

impl DirectoryChild {
    fn as_directory_mut(&mut self) -> &mut Directory {
        match self {
            DirectoryChild::Directory(dir) => dir,
            DirectoryChild::File(_) => panic!("File found where directory expected"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct File {
    name: String,
    index: usize,

    size: u64,
}

pub struct CrateFileProvider {
    storage: tar::Archive<flate2::bufread::GzDecoder<std::io::BufReader<std::fs::File>>>,
    tree: Tree,
}

impl CrateFileProvider {
    pub fn new(path: impl AsRef<Path>) -> Result<CrateFileProvider, std::io::Error> {
        fn inner(path: &std::path::Path) -> Result<CrateFileProvider, std::io::Error> {
            let file = std::fs::File::open(path)?;
            let buf_reader = std::io::BufReader::new(file);
            let gz_decoder = flate2::bufread::GzDecoder::new(buf_reader);
            let storage = tar::Archive::new(gz_decoder);
            Ok(CrateFileProvider {
                storage,
                tree: Tree::new(),
            })
        }
        inner(path.as_ref())
    }
}

pub struct DirChildIter {
    inner: std::collections::hash_map::IntoValues<OsString, DirectoryChild>,
}

impl Iterator for DirChildIter {
    type Item = DirChild<PathBuf>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|v| match v {
            DirectoryChild::Directory(dir) => DirChild::Dir(PathBuf::from(dir.name)),
            DirectoryChild::File(file) => DirChild::File(PathBuf::from(file.name)),
        })
    }
}

impl FuseFsImp for CrateFileProvider {
    type DirListing = DirChildIter;

    type FileContents = std::io::Cursor<Vec<u8>>;

    type Path = PathBuf;

    fn init(&mut self) -> Result<Self::Path, libc::c_int> {
        self.tree.fill_tree(&mut self.storage);
        dbg!(&self.tree);
        Ok(PathBuf::from("/"))
    }

    fn list_files(&mut self, path: Self::Path) -> Option<Self::DirListing> {
        self.tree.get(&path).map(|child| match child {
            DirectoryChild::Directory(dir) => DirChildIter {
                inner: dir.into_iter(),
            },
            DirectoryChild::File(_) => panic!("File found where directory expected"),
        })
    }

    fn read_file(&mut self, path: Self::Path) -> Self::FileContents {
        todo!()
    }
}
