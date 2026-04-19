use rbx_rsml::RsmlCompiler;

fn main() {
    let source = r#"
        Foo!(), Frame {
            Padding!(0% + .5);
        }
    "#;

    let compiled = RsmlCompiler::from_source(source);
    println!("{:#?}", compiled);
}
