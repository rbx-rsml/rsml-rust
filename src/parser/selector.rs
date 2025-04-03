use crate::lexer::Token;

pub struct Selector {
    pub content: String,
    current_token: Token
}

impl<'a> Selector {
    pub fn new(content: &str, token: Token) -> Self {
        Self {
            content: content.to_string(),
            current_token: token
        }
    }

    pub fn append(&mut self, slice: &str, token: Token) {
        let last_token = self.current_token;

        let should_add_space = !matches!(token, Token::Comma | Token::StateOrEnumIdentifier) && matches!(last_token,
             Token::ScopeToDescendants | Token::ScopeToChildren | Token::Text | Token::Comma
        );

        self.current_token = token;

        if should_add_space {
            self.content += &format!(" {}", slice)
        } else {
            self.content += slice
        }
    }
}