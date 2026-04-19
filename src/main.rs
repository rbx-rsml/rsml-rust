use rbx_rsml::{RsmlCompiler, RsmlLexer, RsmlParser};

fn main() {
    let source = r#"
        Foo!(), Frame {
            Padding!(0% + .5);
        }
    "#;

    let lexer = RsmlLexer::new(source);
    let parsed = RsmlParser::new(lexer);
    let compiled = RsmlCompiler::new(parsed);
    println!("{:#?}", compiled);
}
