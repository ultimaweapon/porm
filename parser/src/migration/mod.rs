//! Interface to migration script.
use std::path::PathBuf;

/// Encapsulates migration script.
pub trait Migration: Sized {
    /// Returns name of this migration.
    fn name(&self) -> Option<String>;

    /// Read all statements for the migration.
    fn read(self) -> Result<String, Box<dyn std::error::Error>>;
}

impl Migration for &str {
    #[inline]
    fn name(&self) -> Option<String> {
        None
    }

    #[inline]
    fn read(self) -> Result<String, Box<dyn std::error::Error>> {
        Ok(self.into())
    }
}

impl Migration for PathBuf {
    fn name(&self) -> Option<String> {
        self.file_name().map(|v| v.to_string_lossy().into_owned())
    }

    fn read(self) -> Result<String, Box<dyn std::error::Error>> {
        let v = std::fs::read_to_string(self)?;

        Ok(v)
    }
}
