use crate::types::{Diagnostic, Range, Severity};

use crate::lexer::{SpannedToken, Token};
use crate::parser::parse_error::ParseError;
use crate::parser::Parser;

pub(super) type Trivia<'a> = Vec<SpannedToken<'a>>;

#[derive(Debug)]
pub struct Node<'a> {
    pub token: SpannedToken<'a>,
    pub leading_trivia: Option<Trivia<'a>>
}

pub(super) trait UpdateLastTokenEnd {
    fn update_last_token_end(self, parser: &mut Parser) -> Self;
}

impl<'a> UpdateLastTokenEnd for Option<Node<'a>> {
    fn update_last_token_end(self, parser: &mut Parser) -> Self {
        if let Some(Node { token: SpannedToken (_, _, end), .. }) = self {
            parser.last_token_end = end
        };

        self
    }
}

pub(super) trait ToStatus<'a> {
    fn to_status(self) -> NodeStatus<'a>;
}

impl<'a> ToStatus<'a> for Option<Node<'a>> {
    fn to_status(self) -> NodeStatus<'a> {
        match self {
            Some(node) => NodeStatus::Err(node),
            None => NodeStatus::None
        }
    }
}

impl<'a> ToStatus<'a> for Node<'a> {
    fn to_status(self) -> NodeStatus<'a> {
        NodeStatus::Err(self)
    }
}

pub(super) struct Parsed<'a, T = Construct<'a>> (pub Option<Node<'a>>, pub Option<T>);

impl<'a> Parsed<'a> {
    #[inline(always)]
    pub(super) fn none() -> Self {
        Self (None, None)
    }

    pub(super) fn handle_construct(
        self,
        ast: &mut Vec<Construct<'a>>
    ) -> Option<Node<'a>> {
        if let Some(construct) = self.1 { ast.push(construct) };
        return self.0
    }

}

pub(super) trait SpanEnd {
    fn end(&self) -> usize;
}

#[derive(Debug)]
pub enum SelectorNode<'a> {
    Token(Node<'a>),
    MacroCall {
        name: Node<'a>,
        body: Option<Delimited<'a>>,
    },
}

impl<'a> SelectorNode<'a> {
    pub fn start(&self) -> usize {
        match self {
            Self::Token(node) => node.token.start(),
            Self::MacroCall { name, .. } => name.token.start(),
        }
    }
}

impl<'a> SpanEnd for SelectorNode<'a> {
    fn end(&self) -> usize {
        match self {
            Self::Token(node) => node.token.end(),
            Self::MacroCall { name, body } => {
                if let Some(body) = body { return body.end() }
                name.token.end()
            }
        }
    }
}

#[derive(Debug)]
pub enum MacroBodyContent<'a> {
    Construct(Option<Vec<Construct<'a>>>),
    Assignment(Option<Box<Construct<'a>>>),
    Selector(Option<Vec<SelectorNode<'a>>>),
}

#[derive(Debug)]
pub struct MacroBody<'a> {
    pub open: Node<'a>,
    pub content: MacroBodyContent<'a>,
    pub close: Option<Node<'a>>,
}

impl<'a> SpanEnd for MacroBody<'a> {
    fn end(&self) -> usize {
        if let Some(close) = &self.close { return close.token.end() }
        match &self.content {
            MacroBodyContent::Construct(Some(items)) => {
                if let Some(last) = items.last() { return last.end() }
            },
            MacroBodyContent::Assignment(Some(item)) => return item.end(),
            MacroBodyContent::Selector(Some(items)) => {
                if let Some(last) = items.last() { return last.end() }
            },
            _ => {}
        }
        self.open.token.end()
    }
}

#[derive(Debug)]
pub enum Construct<'a> {
    Macro {
        declaration: Node<'a>,
        name: Option<Node<'a>>,
        args: Option<Delimited<'a>>,
        return_type: Option<(Node<'a>, Option<Node<'a>>)>,
        body: Option<MacroBody<'a>>
    },

    MacroCall {
        name: Node<'a>,
        body: Option<Delimited<'a>>,
        terminator: Option<Node<'a>>
    },

    Derive {
        declaration: Node<'a>,
        body: Option<Box<Construct<'a>>>,
        terminator: Option<Node<'a>>
    },

    Priority {
        declaration: Node<'a>,
        body: Option<Box<Construct<'a>>>,
        terminator: Option<Node<'a>>
    },

    Name {
        declaration: Node<'a>,
        body: Option<Box<Construct<'a>>>,
        terminator: Option<Node<'a>>
    },

    Tween {
        declaration: Node<'a>,
        name: Option<Node<'a>>,
        body: Option<Box<Construct<'a>>>,
        terminator: Option<Node<'a>>
    },

    Rule {
        selectors: Option<Vec<SelectorNode<'a>>>,
        body: Option<Delimited<'a>>
    },

    Assignment {
        left: Node<'a>,
        middle: Option<Node<'a>>,
        right: Option<Box<Construct<'a>>>,
        terminator: Option<Node<'a>>
    },

    MathOperation {
        left: Box<Construct<'a>>,
        operators: Vec<Node<'a>>,
        right: Option<Box<Construct<'a>>>
    },

    AnnotatedTable {
        annotation: Node<'a>,
        body: Option<Delimited<'a>>
    },

    Table {
        body: Delimited<'a>
    },

    Enum {
        keyword: Node<'a>,
        name: Option<Node<'a>>,
        variant: Option<Node<'a>>
    },

    Node { node: Node<'a> },

    None { node: Node<'a> },
}

impl<'a> Construct<'a> {
    pub fn rule(selectors: Option<Vec<SelectorNode<'a>>>, body: Delimited<'a>) -> Self {
        Self::Rule { selectors, body: Some(body) }
    }

    pub fn name_plural(&self) -> &str {
        match self {
            Self::Macro { .. } => "Macros",
            Self::MacroCall { .. } => "Macro calls",
            Self::Derive { .. } => "Derives",
            Self::Priority { .. } => "Priorities",
            Self::Name { .. } => "Names",
            Self::Tween { .. } => "Tweens",
            Self::Rule { .. } => "Rules",
            Self::Assignment { left, .. } => match left.token.value() {
                Token::Identifier(_) => "Property assignments",
                Token::StaticTokenIdentifier(_) => "Static token assignments",
                Token::TokenIdentifier(_) => "Token assignments",
                _ => "Assignments"
            },
            Self::MathOperation { .. } => "Math Operations",
            Self::Table { .. } | Self::AnnotatedTable { .. } => "Tables",
            Self::Enum { .. } => "Enums",
            Self::Node { .. } | Self::None { .. } => "These"
        }
    }

    pub fn start(&self) -> usize {
        match self {
            Self::Macro { declaration, .. } => declaration.token.start(),
            Self::MacroCall { name, .. } => name.token.start(),

            Self::Derive { declaration, .. } |
            Self::Priority { declaration, .. } |
            Self::Name { declaration, .. } |
            Self::Tween { declaration, .. } => declaration.token.start(),

            Self::Rule { selectors, body } => {
                if let Some(selectors) = selectors {
                    if let Some(first) = selectors.first() { return first.start() }
                }
                if let Some(body) = body { return body.start() }
                0
            },

            Self::Assignment { left, .. } => left.token.start(),
            Self::MathOperation { left, .. } => left.start(),
            Self::AnnotatedTable { annotation, .. } => annotation.token.start(),
            Self::Table { body } => body.start(),
            Self::Enum { keyword, .. } => keyword.token.start(),
            Self::Node { node } | Self::None { node } => node.token.start(),
        }
    }

    pub fn span(&self) -> (usize, usize) {
        (self.start(), self.end())
    }
}

impl<'a> SpanEnd for Construct<'a> {
    fn end(&self) -> usize {
        match self {
            Self::Macro { declaration, name, args, return_type, body } => {
                if let Some(x) = body { return x.end() }
                if let Some((arrow, ident)) = return_type {
                    if let Some(ident) = ident { return ident.token.end() }
                    return arrow.token.end()
                }
                if let Some(x) = args { return x.end() }
                if let Some(x) = name { return x.token.end() }
                declaration.token.end()
            },

            Self::MacroCall { name, body, terminator } => {
                if let Some(x) = terminator { return x.token.end() }
                if let Some(x) = body { return x.end() }
                name.token.end()
            }

            Self::Derive { declaration, body, terminator } |
            Self::Priority { declaration, body, terminator } |
            Self::Name { declaration, body, terminator } => {
                if let Some(x) = terminator { return x.token.end() }
                if let Some(x) = body { return x.end() }
                declaration.token.end()
            },

            Self::Tween { declaration, name, body, terminator } => {
                if let Some(x) = terminator { return x.token.end() }
                if let Some(x) = body { return x.end() }
                if let Some(x) = name { return x.token.end() }
                declaration.token.end()
            },

            Self::Rule { body, .. } => {
                if let Some(x) = body { return x.end() }
                0
            },

            Self::Assignment { left, middle, right, terminator } => {
                if let Some(x) = terminator { return x.token.end() }
                if let Some(x) = right { return x.end() }
                if let Some(x) = middle { return x.token.end() }
                left.token.end()
            },

            Self::MathOperation { left, operators, right, .. } => {
                if let Some(x) = right { return x.end() }
                if let Some(x) = operators.last() { return x.token.end() }
                left.end()
            }

            Self::AnnotatedTable { annotation, body } => {
                if let Some(x) = body { return x.end() }
                annotation.token.end()
            },
            Self::Table { body } => body.end(),

            Self::Enum { keyword, name, variant } => {
                if let Some(x) = variant { return x.token.end() }
                if let Some(x) = name { return x.token.end() }
                keyword.token.end()
            }

            Self::Node { node } | Self::None { node } => node.token.end(),
        }
    }
}

pub(super) enum ParseEndedReason {
    Eof,
    Manual
}

#[derive(Debug)]
pub struct Delimited<'a, T: SpanEnd = Construct<'a>> {
    pub left: Node<'a>,
    pub content: Option<Vec<T>>,
    pub right: Option<Node<'a>>
}

impl<'a, T: SpanEnd> Delimited<'a, T> {
    pub(super) fn new(
        left: Node<'a>,
        content: Option<Vec<T>>,
        right: Option<Node<'a>>
    ) -> Self {
        Self { left, content, right }
    }

    #[inline(always)]
    pub(super) fn start(&self) -> usize {
        self.left.token.start()
    }

    pub(super) fn end(&self) -> usize {
        if let Some(x) = &self.right { return x.token.2 }
        if let Some(x) = &self.content {
            if let Some(x) = x.last() { return x.end() }
        }
        self.left.token.end()
    }

    pub(super) fn span(&self) -> (usize, usize) {
        (self.start(), self.end())
    }
}

impl<'a, T: SpanEnd> SpanEnd for Delimited<'a, T> {
    fn end(&self) -> usize {
        Delimited::end(self)
    }
}

#[derive(Debug)]
pub struct AstErrors(pub Vec<Diagnostic>);

impl AstErrors {
    pub fn new() -> Self {
        Self(Vec::new())
    }
}

pub(super) trait PushParseError {
    fn push(&mut self, error: ParseError, range: Range);
}

impl PushParseError for AstErrors {
    fn push(&mut self, error: ParseError, range: Range) {
        self.0.push(Diagnostic {
            range,
            severity: error.severity(),
            code: error.to_string(),
            message: error.message(),
            data: error.data(),
        });
    }
}

#[inline(always)]
pub(super) fn clamp_span_to_end(span_end: usize) -> (usize, usize) {
    (span_end - 1, span_end)
}

#[derive(Debug)]
pub enum NodeStatus<'a> {
    Exists,

    None,

    /// Error node when advancing until a specific token
    /// but a block delimiter token was reached instead.
    Err(Node<'a>),
}

impl<'a> NodeStatus<'a> {
    pub(super) fn consume_err_or_advance(self, parser: &mut Parser<'a>) -> Option<Node<'a>> {
        match self {
            Self::Err(node) => Some(node),
            Self::Exists => parser.advance(),
            Self::None => None
        }
    }
}
