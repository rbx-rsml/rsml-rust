use logos::Logos;

#[derive(Clone, PartialEq, Debug)]
pub struct LexerError(pub String);

#[derive(Logos, Debug, Eq, Hash, PartialEq, Copy, Clone)]
pub enum Token {
    // Do not change the order of the operators.
    #[token("^")]
    OpPow,
    #[token("/")]
    OpDiv,
    #[token("//")]
    OpFloorDiv,
    #[regex(r"[\n\f\t\r ]+\%")]
    OpMod,
    #[token("%", priority = 5)]
    ScaleOrOpMod,
    #[token("*")]
    OpMult,
    #[token("+")]
    OpAdd,
    #[token("-")]
    OpSub,


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

    #[token("true")]
    BoolTrue,

    #[token("false")]
    BoolFalse,

    #[token("nil")]
    Nil,

    #[token("Enum")]
    EnumKeyword,

    #[regex(r"(?i)tw:[a-z]+(:\d+)?")]
    ColorTailwind,

    #[regex(r"(?i)bc:[a-z]+")]
    ColorBrick,

    #[regex(r"(?i)css:[a-z]+")]
    ColorCss,

    #[regex(r"#[0-9a-fA-F]+")]
    ColorHex,

    #[regex(r"\d*\.?\d+", priority = 4)]
    Number,

    #[token("px", priority = 45)]
    Offset,

    #[token(".")]
    TagOrEnumIdentifier,

    #[regex(r#""[^\"\n\t]*""#)]
    #[regex(r#"'[^\'\n\t]*'"#)]
    StringSingle,

    #[regex(r"rbxassetid://\d+")]
    #[regex(r"(rbxasset|rbxthumb|rbxgameasset|rbxhttp|rbxtemp|https?)://[^) ]*")]
    RobloxAsset,

    #[regex(r"contentid://\d+", priority = 999)]
    RobloxContent,

    #[regex(r"[_a-zA-Z][_A-Za-z0-9]*", priority = 0)]
    Text
}

pub fn lex_rsml<'a>(content: &'a str) -> logos::Lexer<'a, Token> {
    Token::lexer(&content)
}