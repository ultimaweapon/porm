use std::convert::Infallible;

/// Provides migration script from [str].
pub struct StrProvider<I>(I);

impl<I> StrProvider<I> {
    /// Create [StrProvider] from a list of [str].
    #[inline]
    pub fn new(list: impl IntoIterator<IntoIter = I>) -> Self {
        Self(list.into_iter())
    }
}

impl<'a, I: Iterator<Item = &'a str>> Iterator for StrProvider<I> {
    type Item = Result<&'a str, Infallible>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(Ok)
    }
}
