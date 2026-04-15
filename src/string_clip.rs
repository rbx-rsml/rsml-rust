pub trait StringClip {
    fn clip<'a>(&'a self, start: usize, end: usize) -> &'a str;
}

impl StringClip for str {
    fn clip<'a>(&'a self, start: usize, end: usize) -> &'a str {
        &self[start..self.len() - end]
    }
}
