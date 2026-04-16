use rbx_rsml::compiler::{Compiler, CompilerData};
use rbx_rsml::lexer::Lexer;
use rbx_rsml::parser::Parser;

fn main() {
    let source = r#"
        $token = "hello";

        Frame {
            @priority 1;
            @name "main-frame";

            BackgroundColor3 = #FF0000;
            Size = udim2(-20px + 100%, -20px + 100%);
            Transparency = 50%;

            > TextLabel {
                Text = "Hello, World!";
                TextColor3 = tw:blue:500;
                TextSize = 24;
            }
        }
    "#;

    let lexer = Lexer::new(source);
    let parsed = Parser::new(lexer);
    let CompilerData { tree_nodes, .. } = Compiler::new(parsed, source);

    println!("{:#?}", tree_nodes);
}
