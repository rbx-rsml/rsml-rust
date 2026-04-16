use rbx_rsml::compiler::{Compiler, CompilerData};
use rbx_rsml::lexer::Lexer;
use rbx_rsml::parser::Parser;

fn main() {
    let source = r#"
        $!Hello = 50px;
        $Size = udim2($!Hello, -20px + 100%);
    "#;

    let lexer = Lexer::new(source);
    let parsed = Parser::new(lexer);
    let CompilerData { tree_nodes, .. } = Compiler::new(parsed);

    println!("{:#?}", tree_nodes);
}
