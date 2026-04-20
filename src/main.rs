use rbx_rsml::RsmlCompiler;

fn main() {
    let source = r#"
        TextLabel {
            FontFace = udim (.5, 50) * 2;
        }
    "#;

    let compiled = RsmlCompiler::from_source(source);
    println!("{:#?}", compiled);
}
