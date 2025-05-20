use logos::Logos;

#[derive(Clone, PartialEq, Debug)]
pub struct LexerError(pub String);

#[derive(Logos, Debug, Eq, Hash, PartialEq, Copy, Clone)]
pub enum UtilsToken {
    #[regex(r"\-\-\[=*\[", priority = 99)]
    CommentMultiStart,

    #[regex(r"\]=*\]", priority = 98)]
    StringMultiEnd,

    #[regex(r"\-\-[^\[\n\f\r]*", priority = 98)]
    CommentSingle,

    #[token("{", priority = 1)]
    ScopeOpen,

    #[token("}", priority = 1)]
    ScopeClose,

    #[token("@util")]
    UtilDeclaration,

    #[regex(r"[_a-zA-Z][-_A-Za-z\d]*", priority = 0)]
    Text
}

pub fn lex_rsml_utils<'a>(content: &'a str) -> logos::Lexer<'a, UtilsToken> {
    UtilsToken::lexer(&content)
}