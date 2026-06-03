use std::io::{Stdout, Write};

/// Provides method to log database migration.
pub trait Logger {
    /// Called before create a migrations history table.
    fn create_history_table(&mut self, name: &str);

    /// Called before migrations start.
    fn start(&mut self, current: Option<usize>);

    /// Called before apply a migration.
    fn run(&mut self, name: &'static str);
}

impl Logger for Stdout {
    fn create_history_table(&mut self, name: &str) {
        writeln!(self, "Creating table '{name}' for migrations history.").unwrap();
    }

    fn start(&mut self, current: Option<usize>) {
        if let Some(v) = current {
            writeln!(self, "Current database version is {v}.").unwrap();
        }
    }

    fn run(&mut self, name: &'static str) {
        writeln!(self, "Applying migration {name}.").unwrap();
    }
}

impl Logger for () {
    #[inline]
    fn create_history_table(&mut self, _: &str) {}

    #[inline]
    fn start(&mut self, _: Option<usize>) {}

    #[inline]
    fn run(&mut self, _: &'static str) {}
}
