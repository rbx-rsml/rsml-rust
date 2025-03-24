use logos::Logos;

#[derive(Clone, PartialEq, Debug)]
pub struct LexerError(pub String);

#[derive(Logos, Debug, Eq, Hash, PartialEq, Copy, Clone)]
#[logos(error = String)]
pub enum Token<'a> {
    #[regex(r"\-\-\[=*\[", priority = 99)]
    CommentMultiStart(&'a str),

    #[regex(r"\[=*\[", priority = 98)]
    StringMultiStart(&'a str),

    #[regex(r"\]=*\]", priority = 98)]
    StringMultiEnd(&'a str),

    #[regex(r"\-\-[^\[\n\f\r]*", priority = 98)]
    CommentSingle(&'a str),

    #[token("{", priority = 1)]
    ScopeOpen,

    #[token("}", priority = 1)]
    ScopeClose,

    #[token("(", priority = 1)]
    ParensOpen,

    #[token(")", priority = 1)]
    ParensClose,

    #[token(",", priority = 1)]
    Comma,

    #[token(";", priority = 1)]
    SemiColon,

    #[token("=", priority = 1)]
    Equals,

    #[token("$", priority = 1)]
    AttributeIdentifier,

    #[token("#", priority = 1)]
    NameIdentifier,

    #[token("::", priority = 1)]
    PsuedoIdentifier,

    #[token(":", priority = 1)]
    StateOrEnumIdentifier,

    #[token(">>", priority = 1)]
    ScopeToDescendants,

    #[token(">", priority = 1)]
    ScopeToChildren,

    #[token("@priority", priority = 1)]
    PriorityDeclaration,

    #[token("@derive", priority = 1)]
    DeriveDeclaration,

    #[token("@name", priority = 1)]
    NameDeclaration,

    #[token("+")]
    OpAdd,

    #[token("-")]
    OpSub,

    #[token("*")]
    OpMult,

    #[token("/")]
    OpDiv,

    #[token("//")]
    OpFloorDiv,

    #[token("^")]
    OpPow,

    #[regex(r"[\n\f\t\r ]+\%")]
    OpMod,

    #[token("true")]
    BoolTrue,

    #[token("false")]
    BoolFalse,

    #[token("Enum")]
    EnumKeyword,

    #[regex(r"(?i)tw:[a-z]+(:\d+)?")]
    ColorTailwind(&'a str),

    #[regex(r"(?i)bc:[a-z]+")]
    ColorBrick(&'a str),

    #[regex(r"(?i)css:[a-z]+")]
    ColorCss(&'a str),

    #[regex(r"#[0-9a-fA-F]+")]
    ColorHex(&'a str),

    #[regex(r"\d*\.?\d+", priority = 4)]
    Number(&'a str),

    #[token("px", priority = 45)]
    Offset,

    #[token("%", priority = 5)]
    ScaleOrOpMod,

    #[token(".")]
    TagOrEnumIdentifier,

    #[regex(r#""[^\"\n\t]*""#)]
    #[regex(r#"'[^\'\n\t]*'"#)]
    StringSingle(&'a str),

    #[regex(r"rbxassetid://\d+")]
    #[regex(r"(rbxasset|rbxthumb|rbxgameasset|rbxhttp|rbxtemp|https?)://[^) ]*")]
    RobloxAsset(&'a str),

    #[regex(r"contentid://\d+", priority = 999)]
    RobloxContent(&'a str),

    #[regex(r"[_a-zA-Z][_A-Za-z0-9]*", priority = 0)]
    Text(&'a str)
}

pub fn lex_rsml<'a>(content: &'a str) -> logos::Lexer<'a, Token<'a>> {
    Token::lexer(&content)
}