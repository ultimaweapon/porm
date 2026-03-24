//! Interface to migration script.
pub use self::str::*;

use std::convert::Infallible;

mod str;

/// Encapsulates migration script.
pub trait Migration: Sized {
    /// Error will be produced by [Self::read()].
    type Err: std::error::Error + 'static;

    /// Returns name of this migration.
    fn name(&self) -> Option<String>;

    /// Read all statements for the migration.
    fn read(self) -> Result<String, Self::Err>;
}

impl Migration for &str {
    type Err = Infallible;

    #[inline]
    fn name(&self) -> Option<String> {
        None
    }

    #[inline]
    fn read(self) -> Result<String, Self::Err> {
        Ok(self.into())
    }
}
