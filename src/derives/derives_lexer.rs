use logos::Logos;

#[derive(Clone, PartialEq, Debug)]
pub struct LexerError(pub String);

#[derive(Logos, Debug, Eq, Hash, PartialEq, Copy, Clone)]
pub enum DerivesToken {
    #[regex(r"\-\-\[=*\[", priority = 99)]
    CommentMultiStart,

    #[regex(r"\[=*\[", priority = 98)]
    StringMultiStart,

    #[regex(r"\]=*\]", priority = 98)]
    StringMultiEnd,

    #[regex(r"\-\-[^\[\n\f\r]*", priority = 98)]
    CommentSingle,

    #[token("{", priority = 1)]
    ScopeOpen,

    #[token("}", priority = 1)]
    ScopeClose,

    #[token("(", priority = 1)]
    ParensOpen,

    #[token(")", priority = 1)]
    ParensClose,

    #[token("@derive")]
    DeriveDeclaration,

    #[regex(r#""[^\"\n\t]*""#)]
    #[regex(r#"'[^\'\n\t]*'"#)]
    StringSingle,

    #[token(",", priority = 1)]
    Comma,

    #[token(";", priority = 1)]
    SemiColon,

    #[regex(r"[_a-zA-Z][-_A-Za-z\d]*", priority = 0)]
    Text
}

pub fn lex_rsml_derives<'a>(content: &'a str) -> logos::Lexer<'a, DerivesToken> {
    DerivesToken::lexer(&content)
}