pub use self::printable::*;

use std::io::{Error, Write};

mod printable;

/// Provides methods to write generated code.
pub struct Writer<T> {
    out: T,
    indent: u8,
}

impl<T> Writer<T> {
    pub fn new(out: T) -> Self {
        Self { out, indent: 0 }
    }
}

impl<T: Write> Writer<T> {
    pub fn increase_indent(&mut self) {
        self.indent += 1;
    }

    pub fn decrease_indent(&mut self) {
        self.indent = self.indent.strict_sub(1);
    }

    pub fn begin(&mut self, v: impl Printable) -> Result<(), Error> {
        for _ in 0..self.indent {
            self.out.write_all(b"    ")?;
        }

        v.print(&mut self.out)
    }

    pub fn append(&mut self, v: impl Printable) -> Result<(), Error> {
        v.print(&mut self.out)
    }

    pub fn end(&mut self, v: impl Printable) -> Result<(), Error> {
        v.print(&mut self.out)?;

        self.out.write_all(b"\n")?;

        Ok(())
    }

    pub fn line(&mut self, v: impl Printable) -> Result<(), Error> {
        for _ in 0..self.indent {
            self.out.write_all(b"    ")?;
        }

        self.end(v)
    }

    pub fn blank_line(&mut self) -> Result<(), Error> {
        writeln!(self.out)
    }
}
