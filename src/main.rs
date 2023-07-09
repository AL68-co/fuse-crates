#![feature(int_roundings)]

use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    time::{Duration, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use fuser::{FileAttr, FileType, Filesystem, MountOption};
use libc::O_TRUNC;
use log::{error, info, warn};

const DIR_FH: u64 = 200679;
const FIL_FH: u64 = 220705;
const BLOCK_SIZE: u32 = 512;

fn main() -> Result<()> {
    env_logger::init();
    match std::process::Command::new("fusermount3")
        .args(["-u", "mount"])
        .status()
    {
        Ok(_) => info!("Mount path successfully unmounted"),
        Err(_) => info!("Did not unmount mount point, maybe it was already unmounted"),
    }
    let fs = FuseFs::new(".");
    fuser::mount2(
        fs,
        "./mount",
        &[
            MountOption::Sync,
            MountOption::DirSync,
            MountOption::NoExec,
            MountOption::RO,
            MountOption::NoAtime,
            MountOption::NoDev,
            MountOption::NoSuid,
        ],
    )?;
    Ok(())
}

#[derive(Debug)]
struct Inode {
    attrs: FileAttr,
    children: Vec<u64>,
    name: OsString,
}

#[derive(Debug)]
struct FuseFs {
    path: PathBuf,
    inodes: BTreeMap<u64, Inode>,
    next_inode: u64,
}

impl FuseFs {
    fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            inodes: BTreeMap::new(),
            next_inode: fuser::FUSE_ROOT_ID + 1,
        }
    }

    fn next_inode(&mut self) -> u64 {
        let ret = self.next_inode;
        self.next_inode += 1;

        ret
    }

    fn populate_crate(&mut self, crate_name: OsString) -> Result<()> {
        let crate_file_path = self.path.join({
            let mut c = crate_name.clone();
            c.push(".crate");
            c
        });
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(
            std::fs::File::open(crate_file_path.clone()).context("Opening file")?,
        ));
        for entry in archive.entries().context("Get entries")? {
            let entry = entry.context("Unwrapping entry")?;
            let entry_path = entry.path().context("Extracting path entry")?;
            println!("{}", entry_path.display());
            let components = entry_path.components().collect::<Vec<_>>();
            let components_length = components.len();
            let mut last_inode = fuser::FUSE_ROOT_ID;
            for component in &components[0..components_length - 1] {
                let last_last_inode = last_inode;
                for child_inode in &self.inodes.get(&last_inode).unwrap().children {
                    if self.inodes.get(child_inode).unwrap().name == component.as_os_str() {
                        last_inode = *child_inode;
                        break;
                    }
                }
                if last_inode == last_last_inode {
                    let new_inode = self.next_inode();
                    let new_inode_object = Inode {
                        attrs: FileAttr {
                            ino: new_inode,
                            ..Self::DIR_ATTR_TEMPLATE
                        },
                        children: vec![],
                        name: component.as_os_str().to_os_string(),
                    };
                    self.inodes.insert(new_inode, new_inode_object);
                    self.inodes
                        .get_mut(&last_inode)
                        .unwrap()
                        .children
                        .push(new_inode);
                }
            }
            let file_name = components.last().unwrap().as_os_str();
            let file_size = entry.header().size().context("File size")?;
            let new_inode = self.next_inode();
            let new_inode_object = Inode {
                attrs: FileAttr {
                    ino: new_inode,
                    size: file_size,
                    blocks: file_size.div_ceil(BLOCK_SIZE.into()),
                    ..Self::FIL_ATTR_TEMPLATE
                },
                children: vec![],
                name: file_name.to_os_string(),
            };
            self.inodes.insert(new_inode, new_inode_object);
            self.inodes
                .get_mut(&last_inode)
                .unwrap()
                .children
                .push(new_inode);
        }
        Ok(())
    }

    const DIR_ATTR_TEMPLATE: FileAttr = FileAttr {
        ino: 0,
        size: 0,
        blocks: 0,
        atime: UNIX_EPOCH, // 1970-01-01 00:00:00
        mtime: UNIX_EPOCH,
        ctime: UNIX_EPOCH,
        crtime: UNIX_EPOCH,
        kind: FileType::Directory,
        perm: 0o555,
        nlink: 2,
        uid: 1062,
        gid: 1063,
        rdev: 0,
        flags: 0,
        blksize: 512,
    };

    const FIL_ATTR_TEMPLATE: FileAttr = FileAttr {
        ino: 0,
        size: 0,
        blocks: 0,
        atime: UNIX_EPOCH, // 1970-01-01 00:00:00
        mtime: UNIX_EPOCH,
        ctime: UNIX_EPOCH,
        crtime: UNIX_EPOCH,
        kind: FileType::RegularFile,
        perm: 0o444,
        nlink: 1,
        uid: 1062,
        gid: 1063,
        rdev: 0,
        flags: 0,
        blksize: BLOCK_SIZE,
    };
}

impl Filesystem for FuseFs {
    fn init(
        &mut self,
        _req: &fuser::Request<'_>,
        _config: &mut fuser::KernelConfig,
    ) -> Result<(), libc::c_int> {
        self.inodes.insert(
            fuser::FUSE_ROOT_ID,
            Inode {
                attrs: FileAttr {
                    ino: fuser::FUSE_ROOT_ID,
                    ..Self::DIR_ATTR_TEMPLATE
                },
                children: vec![],
                name: "".to_string().into(),
            },
        );
        for file in std::fs::read_dir(&self.path).unwrap() {
            let file = file.unwrap();
            if file.path().extension() != Some(OsStr::new("crate")) {
                continue;
            }
            let path = file.path();
            let name = path.file_stem().unwrap();
            let inode = self.next_inode();
            let inode_object = Inode {
                attrs: FileAttr {
                    ino: inode,
                    ..Self::DIR_ATTR_TEMPLATE
                },
                children: vec![],
                name: name.to_os_string(),
            };
            self.inodes.insert(inode, inode_object);
            self.inodes
                .get_mut(&fuser::FUSE_ROOT_ID)
                .unwrap()
                .children
                .push(inode);
            log::debug!("Crate found: {}", name.to_string_lossy());
            self.populate_crate(name.to_os_string()).unwrap();
            log::debug!("Crate populated: {}", name.to_string_lossy());
        }
        info!("Init successful!");
        Ok(())
    }

    fn getattr(&mut self, _req: &fuser::Request<'_>, ino: u64, reply: fuser::ReplyAttr) {
        match self.inodes.get(&ino) {
            Some(inode) => reply.attr(&Duration::MAX, &inode.attrs),
            None => reply.error(libc::ENOENT),
        }
    }

    fn opendir(
        &mut self,
        _req: &fuser::Request<'_>,
        ino: u64,
        flags: i32,
        reply: fuser::ReplyOpen,
    ) {
        if flags
            & (libc::O_APPEND
                | libc::O_CREAT
                | libc::O_EXCL
                | libc::O_RDWR
                | libc::O_WRONLY
                | O_TRUNC)
            != 0
        {
            reply.error(libc::EROFS);
            warn!(
                "Opendir failed because flags (0x{:x}) are not correct, ROFS",
                flags
                    & (libc::O_APPEND
                        | libc::O_CREAT
                        | libc::O_EXCL
                        | libc::O_RDWR
                        | libc::O_WRONLY
                        | O_TRUNC)
            );
            return;
        }
        if !self.inodes.contains_key(&ino) {
            reply.error(libc::ENOENT);
            warn!("Opendir failed because inode (0x{ino:x}) does not exist, NOENT");
            return;
        }
        reply.opened(DIR_FH, fuser::consts::FOPEN_KEEP_CACHE)
    }

    fn readdir(
        &mut self,
        _req: &fuser::Request<'_>,
        ino: u64,
        fh: u64,
        offset: i64,
        mut reply: fuser::ReplyDirectory,
    ) {
        if !self.inodes.contains_key(&ino) {
            error!("[readdir], (0x{ino:016x}) ENOENT");
            return reply.error(libc::ENOENT);
        }
        if fh != DIR_FH {
            error!("[readdir], (0x{ino:016x}) ENOBADF");
            return reply.error(libc::EBADF);
        }
        if self.inodes.get(&ino).unwrap().attrs.kind != FileType::Directory {
            return reply.error(libc::ENOTDIR);
        }
        let offset = if offset == 0 { 0 } else { offset + 1 };
        if offset <= 0 {
            if reply.add(ino, 0, FileType::Directory, ".") {
                return reply.ok();
            }
        }
        if offset <= 1 {
            if reply.add(ino, 1, FileType::Directory, "..") {
                return reply.ok();
            }
        }
        let mut offset = 2.max(offset);
        for child_inode in self
            .inodes
            .get(&ino)
            .unwrap()
            .children
            .iter()
            .skip((offset - 2) as usize)
        {
            let kind = self.inodes.get(child_inode).unwrap().attrs.kind;
            let name = &self.inodes.get(child_inode).unwrap().name;
            if reply.add(*child_inode, offset, kind, name) {
                return reply.ok();
            }
            offset += 1;
        }
        return reply.ok();
    }

    fn lookup(
        &mut self,
        _req: &fuser::Request<'_>,
        parent: u64,
        name: &OsStr,
        reply: fuser::ReplyEntry,
    ) {
        if !self.inodes.contains_key(&parent) {
            return reply.error(libc::ENOENT);
        }
        if self.inodes.get(&parent).unwrap().attrs.kind != FileType::Directory {
            return reply.error(libc::ENOTDIR);
        }
        for child_inode in &self.inodes.get(&parent).unwrap().children {
            let tested_name = &self.inodes.get(child_inode).unwrap().name;
            if name != tested_name {
                continue;
            }
            return reply.entry(
                &Duration::MAX,
                &self.inodes.get(child_inode).unwrap().attrs,
                0,
            );
        }
        return reply.error(libc::ENOENT);
    }
}
