mod lexer;
pub use lexer::{lex_rsml, Token};

mod parser;
pub use parser::{parse_rsml, TreeNode, TreeNodeGroup, Datatype};

mod macros;
pub use macros::{lex_rsml_macros, parse_rsml_macros, MacroGroup};

mod derives;
pub use derives::{lex_rsml_derives, parse_rsml_derives};

mod utils;

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