use rbx_rsml::RsmlCompiler;

fn main() {
    let source = r#"
        TextLabel {
            FontFace = font (16777217, :SemiBold, "Italic");
        }
    "#;

    let compiled = RsmlCompiler::from_source(source);
    println!("{:#?}", compiled);
}
