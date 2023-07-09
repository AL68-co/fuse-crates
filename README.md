# fuse-crates

Instead of extracting your crates, store them in a FUSE fs!

This tool is meant for Cargo, and is in a PRE-ALPHA state. Use at your own risk!

## The goal of this tool

This tool has a goal of being able to avoid having to extract crates that are downloaded by Cargo. Those are waste of time and disk space, since the extracted files are already stored in the .crate files that are also stored.
This tool (for now) creates a FUSE filesystem, containing the seemingly extracted contents. It is designed to ba able to be used by Cargo, but I'm not sure how currently.

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
