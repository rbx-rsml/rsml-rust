mod lexer;
pub use lexer::{lex_rsml, Token};

mod parser;
pub use parser::{parse_rsml, TreeNode, TreeNodes};

mod string_clip {
    pub trait StringClip {
        fn clip<'a>(&'a self, start: usize, end: usize) -> &'a str;
    }
    
    impl StringClip for str {
        fn clip<'a>(&'a self, start: usize, end: usize) -> &'a str {
            &self[start..self.len() - end]
        }
    }
}