use logos::Logos;

#[derive(Clone, PartialEq, Debug)]
pub struct LexerError(pub String);

#[derive(Logos, Debug, Eq, Hash, PartialEq, Copy, Clone)]
pub enum MacrosToken {
    #[regex(r"\-\-\[=*\[", priority = 99)]
    CommentMultiStart,

    #[regex(r"\]=*\]", priority = 98)]
    StringMultiEnd,

    #[regex(r"\-\-[^\[\n\f\r]*", priority = 98)]
    CommentSingle,

    #[token("(")]
    ParensOpen,

    #[token(")")]
    ParensClose,

    #[token("{")]
    ScopeOpen,

    #[token("}")]
    ScopeClose,

    #[token("@macro")]
    MacroDeclaration,

    #[regex(r"[_a-zA-Z][-_A-Za-z\d]*", priority = 0)]
    Text
}

pub fn lex_rsml_macros<'a>(content: &'a str) -> logos::Lexer<'a, MacrosToken> {
    MacrosToken::lexer(&content)
}