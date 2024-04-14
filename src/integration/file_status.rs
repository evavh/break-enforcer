use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Seek, Write};
use std::iter;

use color_eyre::eyre::Context;
use color_eyre::Result;

pub struct FileStatus {
    max_len: usize,
    file: fs::File,
}

impl FileStatus {
    pub fn new() -> Result<Self> {
        // use std::os::unix::fs::OpenOptionsExt;
        match std::fs::create_dir("/var/run/break_enforcer") {
            Ok(()) => (),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => (),
            err @ Err(_) => err.wrap_err("Could not create directory for integration file")?,
        }
        // let owner_write_rest_read = 0o422;
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            // .mode(owner_write_rest_read)
            .open("/var/run/break_enforcer/status.txt")
            .wrap_err("Could not create integration file")?;

        Ok(Self { file, max_len: 0 })
    }

    pub fn update(&mut self, msg: &str) {
        self.max_len = self.max_len.max(msg.chars().count());

        // can never shrink file as the reader might read the just truncated
        // file leading to a corrupt message or flickering
        let padded: String = msg
            .chars()
            .chain(iter::repeat(' '))
            .take(self.max_len)
            .collect();
        self.file.seek(std::io::SeekFrom::Start(0)).unwrap();
        self.file.write_all(padded.as_bytes()).unwrap();
    }
}
