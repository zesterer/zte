use crate::{State, state::{Activity, File}};
use slotmap::HopSlotMap;
use std::{
    fs::File as StdFile,
    path::Path,
    io::Read as _,
};

impl State {
    pub fn open(&mut self, path: &Path) -> Result<(), String> {
        match StdFile::open(path) {
            Ok(file) => {
                self.activities.insert(Activity::File(File {
                    views: HopSlotMap::default(),
                }));
                Ok(())
            },
            Err(err) => Err(err.to_string()),
        }
    }
}
