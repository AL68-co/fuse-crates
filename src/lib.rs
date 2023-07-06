#![feature(int_roundings)]

use std::{
    collections::BTreeMap,
    ffi::OsStr,
    io::{Read, Seek},
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    time::UNIX_EPOCH,
};

use fuser::{FileAttr, Filesystem};

pub mod crate_file_provider;

pub struct FuseFs<Imp: FuseFsImp> {
    inodes: BTreeMap<u64, InodeEntry<Imp::Path>>,
    bidirectional_tree_root: Option<InodeTreeItem<Imp::Path>>,
    next_inode: u64,
    imp: Imp,
}

struct InodeTreeItem<P> {
    inode: u64,
    path: P,
    children: Vec<u64>,
    parent: u64,
}

impl<P> InodeTreeItem<P> {
    fn new(inode: u64, path: P, parent: u64, children: Vec<u64>) -> InodeTreeItem<P> {
        InodeTreeItem {
            inode,
            path,
            children,
            parent,
        }
    }
}

pub trait FuseFsImp {
    type DirListing: Iterator<Item = DirChild<Self::Path>>;
    type FileContents: Read + Seek;
    type Path: Clone + Into<PathBuf> + From<PathBuf>;

    /// Returns the root path
    fn init(&mut self) -> Result<Self::Path, libc::c_int>;

    fn list_files(&mut self, path: Self::Path) -> Option<Self::DirListing>;

    fn read_file(&mut self, path: Self::Path) -> Self::FileContents;
}

#[non_exhaustive]
pub enum DirChild<Path> {
    Dir(Path),
    File(Path),
}

impl<Path> DirChild<Path> {
    fn name(&self) -> &Path {
        match self {
            DirChild::Dir(path) => path,
            DirChild::File(path) => path,
        }
    }
}

struct InodeEntry<P> {
    path: P,
    size: u64,
    is_a_dir: bool,
}

impl<Imp: FuseFsImp> FuseFs<Imp> {
    pub fn new(imp: Imp) -> FuseFs<Imp> {
        FuseFs {
            imp,
            next_inode: fuser::FUSE_ROOT_ID + 1,
            bidirectional_tree_root: None,
            inodes: BTreeMap::new(),
        }
    }

    fn populate_inodes(&mut self) {
        let root_path = self.imp.init().unwrap();
        self.inodes.insert(
            fuser::FUSE_ROOT_ID,
            InodeEntry {
                path: root_path.clone(),
                size: 0,
                is_a_dir: true,
            },
        );
        let root_inode_tree = InodeTreeItem::new(
            fuser::FUSE_ROOT_ID,
            root_path.clone(),
            1,
            self.populate_inodes_rec(root_path),
        );
        self.bidirectional_tree_root = Some(root_inode_tree);
    }

    fn populate_inodes_rec(&mut self, path: <Imp as FuseFsImp>::Path) -> Vec<u64> {
        eprintln!(
            "Populating inodes for {:?}",
            Into::<PathBuf>::into(path.clone())
        );
        self.imp
            .list_files(path.clone())
            .unwrap_or_else(|| panic!("Tried to find {:?}", Into::<PathBuf>::into(path.clone())))
            .map(|child| {
                let inode = self.next_inode;
                self.next_inode += 1;
                self.inodes.insert(
                    inode,
                    InodeEntry {
                        path: Into::<PathBuf>::into(path.clone())
                            .join::<PathBuf>(child.name().clone().into())
                            .into(),
                        size: 0,
                        is_a_dir: match child {
                            DirChild::Dir(_) => true,
                            DirChild::File(_) => false,
                        },
                    },
                );
                if let DirChild::Dir(cpath) = child {
                    self.populate_inodes_rec(
                        Into::<PathBuf>::into(path.clone())
                            .join::<PathBuf>(cpath.into())
                            .into(),
                    );
                }
                inode
            })
            .collect()
    }
}

impl<Imp: FuseFsImp> Filesystem for FuseFs<Imp>
where
    <Imp as FuseFsImp>::Path: PartialEq<OsStr>,
{
    fn init(
        &mut self,
        _req: &fuser::Request<'_>,
        _config: &mut fuser::KernelConfig,
    ) -> Result<(), libc::c_int> {
        self.populate_inodes();

        Ok(())
    }

    fn getattr(&mut self, _req: &fuser::Request<'_>, ino: u64, reply: fuser::ReplyAttr) {
        if let Some(entry) = self.inodes.get(&ino) {
            reply.attr(
                &std::time::Duration::from_secs(1),
                &FileAttr {
                    ino,
                    size: entry.size,
                    blocks: entry.size.div_ceil(512),
                    blksize: 512,
                    atime: std::time::SystemTime::now(),
                    mtime: UNIX_EPOCH,
                    ctime: UNIX_EPOCH,
                    crtime: UNIX_EPOCH,
                    kind: if entry.is_a_dir {
                        fuser::FileType::Directory
                    } else {
                        fuser::FileType::RegularFile
                    },
                    perm: if entry.is_a_dir { 0o555 } else { 0o444 },
                    nlink: 1,
                    uid: 0,
                    gid: 0,
                    rdev: 0,
                    flags: 0,
                },
            );
        } else {
            reply.error(libc::ENOENT);
        }
    }

    fn lookup(
        &mut self,
        _req: &fuser::Request<'_>,
        parent: u64,
        name: &OsStr,
        reply: fuser::ReplyEntry,
    ) {
        todo!()
    }

    fn readdir(
        &mut self,
        _req: &fuser::Request<'_>,
        ino: u64,
        fh: u64,
        offset: i64,
        reply: fuser::ReplyDirectory,
    ) {
        todo!()
    }
}
