mod lexer;
use std::{fs, path::Path, sync::LazyLock};

use indexmap::IndexSet;
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

const BUILTINS_CONTENT: &str = include_str!("../builtins.rsml");

static BUILTIN_MACROS: LazyLock<MacroGroup> = LazyLock::new(|| {
    let mut macro_group = MacroGroup::new();
    parse_rsml_macros(&mut macro_group, &mut lex_rsml_macros(&BUILTINS_CONTENT));
    macro_group
});

pub fn file_to_rsml(path: &Path) -> (TreeNodeGroup, IndexSet<String>) {
    let content = fs::read_to_string(path)
        .expect("Could not read the file");

    let mut macro_group = (*BUILTIN_MACROS).clone();

    let parent_path = path.parent().unwrap();

    let derives = parse_rsml_derives(&mut lex_rsml_derives(&content));
    for derive in &derives {
        let derive = if !derive.ends_with(".rsml") { &format!("{}.rsml", derive) } else { derive };
        let derive_path = parent_path.join(derive);

        if let Ok(derive_content) = fs::read_to_string(derive_path) {
            parse_rsml_macros(&mut macro_group, &mut lex_rsml_macros(&derive_content));
        }
    }
    
    parse_rsml_macros(&mut macro_group, &mut lex_rsml_macros(&content));

    (parse_rsml(&mut lex_rsml(&content), &macro_group), derives)
}