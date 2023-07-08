use fuser::MountOption;

fn main() {
    env_logger::init();
    let fs = compressed_dir::FuseFs::new(
        compressed_dir::crate_file_provider::CrateFileProvider::new("cc-1.0.73.crate").unwrap(),
    );
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
    )
    .unwrap();
}
