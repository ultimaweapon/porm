//! Provides types to configure [porm-parser](https://crates.io/crates/porm-parser).
pub use self::pluralizer::*;

mod pluralizer;

/// Control behavior of [porm-parser](https://crates.io/crates/porm-parser).
pub struct Config<'a, 'b: 'a> {
    /// Object to get plural form of English word.
    pub pluralizer: &'a (dyn Pluralizer + 'b),
}
