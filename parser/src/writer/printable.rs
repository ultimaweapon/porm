use std::fmt::Arguments;
use std::io::{Error, Write};

/// Object that can be printed.
pub trait Printable {
    fn print(self, dst: impl Write) -> Result<(), Error>;
}

impl Printable for &str {
    fn print(self, mut dst: impl Write) -> Result<(), Error> {
        dst.write_all(self.as_bytes())
    }
}

impl<'a> Printable for Arguments<'a> {
    fn print(self, mut dst: impl Write) -> Result<(), Error> {
        dst.write_fmt(self)
    }
}
