use std::borrow::Cow;

/// Provides method to pluralize English word.
pub trait Pluralizer {
    /// Returns plural form of the given word.
    fn to_plural<'a>(&self, w: &'a str) -> Cow<'a, str>;
}

/// Implementation of [Pluralizer] by appending "s" to any words.
pub struct SimplePluralizer;

impl Pluralizer for SimplePluralizer {
    fn to_plural<'a>(&self, w: &'a str) -> Cow<'a, str> {
        let mut s = String::with_capacity(w.len() + 1);

        s.push_str(w);
        s.push('s');

        s.into()
    }
}
