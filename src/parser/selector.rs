use crate::lexer::Token;

pub struct Selector<'a> {
    pub content: String,
    current_token: Token<'a>
}

impl<'a> Selector<'a> {
    pub fn new(slice: &str, token: Token<'a>) -> Self {
        Self {
            content: slice.to_string(),
            current_token: token
        }
    }

    pub fn append(&mut self, slice: &str, token: Token<'a>) {
        let should_add_space = !matches!(token, Token::Comma) && matches!(self.current_token,
            Token::ScopeToDescendants | Token::ScopeToChildren | Token::Text(_) | Token::Comma
        );

        self.current_token = token;

        if should_add_space { self.content += " " }
        self.content += slice
    }
}