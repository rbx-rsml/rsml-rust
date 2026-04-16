use crate::lazy_collection;
use crate::string_clip::StringClip;
use enum_kinds::EnumKind;
use logos::{Lexer as LogosLexer, Logos, SpannedIter};
use ropey::Rope;
use std::{
    collections::{HashMap, HashSet},
    mem::{discriminant, Discriminant},
    sync::LazyLock,
};

pub struct Lexer<'a> {
    token_stream: SpannedIter<'a, Token<'a>>,
    pub rope: Rope,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            token_stream: Token::lexer(input).spanned(),
            rope: Rope::from_str(input),
        }
    }

    pub fn slice(&self) -> &'a str {
        self.token_stream.slice()
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = SpannedToken<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let Some((token, span)) = self.token_stream.next() else { return None };

        match token {
            Ok(token) => match token {
                Token::CommentMulti(MultilineString { span, .. })
                | Token::StringMulti(MultilineString { span, .. }) => {
                    Some(SpannedToken::new(span.0, token, span.1))
                }

                _ => Some(SpannedToken::new(span.start, token, span.end)),
            },

            Err(_) => Some(SpannedToken::new(span.start, Token::Error, span.end)),
        }
    }
}


#[derive(Debug, Clone)]
pub struct SpannedToken<'a>(pub usize, pub Token<'a>, pub usize);

impl<'a> SpannedToken<'a> {
    pub fn new(start: usize, value: Token<'a>, end: usize) -> Self {
        Self(start, value, end)
    }

    #[inline(always)]
    pub fn start(&self) -> usize {
        self.0
    }

    #[inline(always)]
    pub fn value(&self) -> &Token<'a> {
        &self.1
    }

    #[inline(always)]
    pub fn end(&self) -> usize {
        self.2
    }

    #[inline(always)]
    pub fn span(&self) -> (usize, usize) {
        (self.0, self.2)
    }
}

fn str_to_option(str: &str) -> Option<&str> {
    if str.len() == 0 {
        None
    } else {
        Some(str)
    }
}

#[derive(Logos, Clone, Debug, PartialEq, EnumKind)]
#[enum_kind(TokenKind, derive(Hash))]
#[logos(skip r"[ \t\n\r\f]+")]
#[logos(subpattern ident = r"[_A-Za-z][_A-Za-z\d]*|[_A-Za-z]+(-[A-Za-z\d_]+)+")]
#[logos(subpattern numsect = r"_*[\d]+_*")]
#[logos(subpattern num = r"((?&numsect)+\.)?(?&numsect)+|\.(?&numsect)")]
pub enum Token<'a> {
    #[regex(r"\-\-\[=*\[", priority = 99, callback = |lex| multiline_string_block_callback(lex, 2))]
    CommentMulti(MultilineString<'a>),

    #[regex(r"\-\-[^(\[\[)].*", priority = 1, callback = |lex| str_to_option(&lex.slice().clip(2, 0)))]
    #[regex(r"\-\-", priority = 1, callback = |_| None::<&str>)]
    CommentSingle(Option<&'a str>),

    // When adding a new declaration make sure to
    // update the `DECLARATIONS` array located above.
    #[token("@derive")]
    DeriveDeclaration,

    #[token("@macro")]
    MacroDeclaration,

    #[token("@priority")]
    PriorityDeclaration,

    #[token("@name")]
    NameDeclaration,

    #[token("@tween")]
    TweenDeclaration,

    #[regex(r"@(?&ident)", callback = |lex| str_to_option(&lex.slice()[1..]))]
    QuerySelector(&'a str),


    #[regex(r"\$!(?&ident)?", callback = |lex| str_to_option(&lex.slice()[2..]))]
    StaticTokenIdentifier(&'a str),

    #[regex(r"\$(?&ident)?", callback = |lex| str_to_option(&lex.slice()[1..]))]
    TokenIdentifier(&'a str),

    #[regex(r"(?&ident)")]
    Identifier(&'a str),

    #[regex(r"&(?&ident)?", callback = |lex| str_to_option(&lex.slice()[1..]))]
    MacroArgIdentifier(Option<&'a str>),

    #[regex(r"(?&ident)!", callback = |lex| str_to_option(&lex.slice().clip(0, 1)))]
    MacroCallIdentifier(Option<&'a str>),

    #[token("=")]
    Equals,

    #[token(",")]
    Comma,

    #[token(";")]
    SemiColon,

    #[regex(r"#(?&ident)", callback = |lex| str_to_option(&lex.slice()[1..]))]
    NameSelector(&'a str),

    #[regex(r"\.(?&ident)?", callback = |lex| str_to_option(&lex.slice()[1..]))]
    TagSelectorOrEnumPart(Option<&'a str>),

    #[regex(r":(?&ident)?", callback = |lex| str_to_option(&lex.slice()[1..]))]
    StateSelectorOrEnumPart(Option<&'a str>),

    #[regex(r"::(?&ident)", callback = |lex| str_to_option(&lex.slice()[2..]))]
    PseudoSelector(&'a str),

    #[token("->")]
    ReturnArrow,

    #[token(">")]
    ChildrenSelector,

    #[token(">>")]
    DescendantsSelector,

    #[token("{")]
    ScopeOpen,

    #[token("}")]
    ScopeClose,

    #[token("(")]
    ParensOpen,

    #[token(")")]
    ParensClose,

    #[token("/")]
    OpDiv,

    #[token("//")]
    OpFloorDiv,

    #[token("%")]
    OpMod,

    #[token("*")]
    OpMult,

    #[token("^")]
    OpPow,

    #[token("+")]
    OpAdd,

    #[token("-")]
    OpSub,

    #[regex(r"\[=*\[", priority = 98, callback = |lex| multiline_string_block_callback(lex, 0))]
    StringMulti(MultilineString<'a>),

    #[regex(r#""[^\"\n\t]*""#, callback = |lex| lex.slice().clip(1, 1))]
    #[regex(r#"'[^\'\n\t]*'"#, callback = |lex| lex.slice().clip(1, 1))]
    StringSingle(&'a str),

    #[regex(r"(?&num)", priority = 99)]
    Number(&'a str),

    #[regex(r"(?&num)%", priority = 99)]
    NumberScale(&'a str),

    #[regex(r"(?&num)px", priority = 99)]
    NumberOffset(&'a str),

    #[token("true")]
    #[token("false")]
    Boolean(&'a str),

    #[token("nil")]
    Nil,

    #[regex(r"(?i)tw:[a-z]+(:\d+)?")]
    ColorTailwind(&'a str),

    #[regex(r"(?i)skin:[a-z]+(:\d+)?")]
    ColorSkin(&'a str),

    #[regex(r"(?i)bc:[a-z]+")]
    ColorBrick(&'a str),

    #[regex(r"(?i)css:[a-z]+")]
    ColorCss(&'a str),

    #[regex(r"#[\da-fA-F]+", priority = 99)]
    ColorHex(&'a str),

    #[regex(r"rbxassetid://\d*")]
    #[regex(r"(rbxasset|rbxthumb|rbxgameasset|rbxhttp|rbxtemp|https?)://[^) ]*")]
    RbxAsset(&'a str),

    #[regex(r"contentid://\d*", priority = 999)]
    RbxContent(&'a str),

    #[token("Enum")]
    EnumKeyword,

    Error,

    None,
}

impl<'a> Token<'a> {
    #[inline(always)]
    pub fn discriminant(&self) -> Discriminant<TokenKind> {
        discriminant(&TokenKind::from(self))
    }

    #[inline(always)]
    pub fn kind(&self) -> TokenKind {
        TokenKind::from(self)
    }
}

impl TokenKind {
    pub fn name(&self) -> &'static str {
        TOKEN_KIND_STRING_MAP
            .get(self)
            .map(|x| *x)
            .unwrap_or_else(|| "**error**")
    }
}

#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip r"[ \t\n\r\f]+")]
enum MultilineStringToken {
    #[regex(r"\]=*\]")]
    ExitMultilineString,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MultilineString<'a> {
    pub nestedness: Result<usize, usize>,
    pub content: &'a str,
    pub span: (usize, usize),
}

fn multiline_string_block_callback<'a>(
    lexer: &mut LogosLexer<'a, Token<'a>>,
    sub_amount: usize,
) -> MultilineString<'a> {
    let mut sub_lexer = lexer.clone().morph::<MultilineStringToken>();

    // Subtracts by `sub_amount` to account for leading characters (typically `--` for multi-line comments).
    // Subtracts by 2 to account for `[` either side of the equal signs.
    let open_nestedness = sub_lexer.slice().len() - sub_amount - 2;
    let open_span_start = sub_lexer.span().start;

    let content_span_start = open_span_start + 2;

    while let Some(token) = sub_lexer.next() {
        match token {
            Ok(MultilineStringToken::ExitMultilineString) => {
                let close_span = sub_lexer.span();
                // Subtracts by 2 to account for `]` either side of the equal signs.
                let close_nestedness = sub_lexer.slice().len() - 2;

                if open_nestedness == close_nestedness {
                    let data = MultilineString {
                        nestedness: Ok(open_nestedness),
                        content: &sub_lexer.source()[content_span_start..close_span.start],
                        span: (open_span_start, close_span.end),
                    };

                    *lexer = sub_lexer.morph();

                    return data;
                }
            }
            _ => {}
        }
    }

    let data = MultilineString {
        nestedness: Err(open_nestedness),
        content: sub_lexer.source().clip(content_span_start, 0),
        span: (open_span_start, sub_lexer.source().len()),
    };

    *lexer = sub_lexer.morph();

    data
}

pub const TOKEN_KIND_CONSTRUCT_DELIMITERS: LazyLock<HashSet<TokenKind>> = lazy_collection! {
    TokenKind::ParensClose,
    TokenKind::ScopeClose,
    TokenKind::SemiColon,

    TokenKind::DeriveDeclaration,
    TokenKind::MacroDeclaration,
    TokenKind::NameDeclaration,
    TokenKind::PriorityDeclaration,
    TokenKind::TweenDeclaration
};

pub const TOKEN_KIND_MACRO_CALL_DELIMITERS: LazyLock<HashSet<TokenKind>> = lazy_collection! {
    TokenKind::ParensClose,
    TokenKind::ScopeClose,
    TokenKind::ScopeOpen,
    TokenKind::SemiColon,

    TokenKind::DeriveDeclaration,
    TokenKind::MacroDeclaration,
    TokenKind::NameDeclaration,
    TokenKind::PriorityDeclaration,
    TokenKind::TweenDeclaration
};

pub const TOKEN_KIND_INSIDE_PARENS_CONSTRUCT_DELIMITERS: LazyLock<HashSet<TokenKind>> = lazy_collection! {
    TokenKind::ParensClose,
};

pub const TOKEN_KIND_ADD_SUB_PRECEDENCE: usize = 0;

pub const TOKEN_KIND_OPERATOR_PRECEDENCE: LazyLock<HashMap<TokenKind, usize>> = lazy_collection! {
    TokenKind::OpDiv => 1,
    TokenKind::OpFloorDiv => 1,
    TokenKind::OpMod => 1,
    TokenKind::OpMult => 1,
    TokenKind::OpPow => 1,
    TokenKind::OpAdd => TOKEN_KIND_ADD_SUB_PRECEDENCE,
    TokenKind::OpSub => TOKEN_KIND_ADD_SUB_PRECEDENCE,
};

const TOKEN_KIND_STRING_MAP: LazyLock<HashMap<TokenKind, &'static str>> = lazy_collection! {
    TokenKind::CommentMulti => "`comment`",
    TokenKind::CommentSingle => "`comment`",
    TokenKind::DeriveDeclaration => "\"@derive\"",
    TokenKind::MacroDeclaration => "\"@macro\"",
    TokenKind::PriorityDeclaration => "\"@priority\"",
    TokenKind::NameDeclaration => "\"@name\"",
    TokenKind::TweenDeclaration => "\"@tween\"",
    TokenKind::QuerySelector => "`query selector`",
    TokenKind::Identifier => "`identifer`",
    TokenKind::MacroArgIdentifier => "`macro argument`",
    TokenKind::MacroCallIdentifier => "`macro call`",
    TokenKind::Equals => "\"=\"",
    TokenKind::Comma => "\",\"",
    TokenKind::SemiColon => "\";\"",
    TokenKind::NameSelector => "`name selector`",
    TokenKind::TagSelectorOrEnumPart => "`tag selector`",
    TokenKind::StateSelectorOrEnumPart => "`state selector`",
    TokenKind::PseudoSelector => "`pseudo selector`",
    TokenKind::ReturnArrow => "\"->\"",
    TokenKind::ChildrenSelector => "\">\"",
    TokenKind::DescendantsSelector => "\">>\"",
    TokenKind::ScopeOpen => "\"{\"",
    TokenKind::ScopeClose => "\"}\"",
    TokenKind::ParensOpen => "\"(\"",
    TokenKind::ParensClose => "\")\"",
    TokenKind::StringMulti => "`string`",
    TokenKind::StringSingle => "`string`",
    TokenKind::Number => "`number`",
    TokenKind::NumberScale => "`udim scale`",
    TokenKind::NumberOffset => "`udim offset`",
    TokenKind::ColorTailwind => "`tailwind color`",
    TokenKind::ColorBrick => "`brick color`",
    TokenKind::ColorCss => "`css color`",
    TokenKind::ColorHex => "`hex color`",
};
