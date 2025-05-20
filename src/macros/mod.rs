mod macros_lexer;
use std::{collections::HashMap, fmt::Debug};

use guarded::guarded_unwrap;
use macro_token_iterator::MacroArgs;
pub use macros_lexer::lex_rsml_macros;

mod macros_parser;
pub use macros_parser::parse_rsml_macros;

mod macro_token_iterator;
pub use macro_token_iterator::MacroTokenIterator;

use crate::{lex_rsml, Token};

pub type TokenPair<TokenType = Token> = (TokenType, String);

#[derive(Debug, Clone)]
pub struct Macro {
    pub token_pairs: Vec<TokenPair>,
    arg_places: Option<HashMap<usize, usize>>
}
impl<'a> Macro {
    fn new(body: String, args: Option<HashMap<&'a str, usize>>) -> Self {
        let mut lexer: logos::Lexer<'_, Token> = lex_rsml(&body);
        
        let mut token_pairs: Vec<TokenPair> = vec![];

        if let Some(args) = args {
            let mut token_pairs_idx = 0usize;
            let mut arg_places: HashMap<usize, usize> = HashMap::new();

            loop {
                let token = guarded_unwrap!(guarded_unwrap!(lexer.next(), break), continue);
                let token_slice = lexer.slice();

                if matches!(token, Token::StaticArgIdentifier) {
                    if let Some(Ok(next_token)) = lexer.next() {
                        let next_token_slice = lexer.slice();

                        if matches!(next_token, Token::Text) {
                            if let Some(arg_idx) = args.get(next_token_slice) {
                                arg_places.insert(token_pairs_idx, *arg_idx);

                            } else {
                                token_pairs.push((Token::Nil, "nil".to_string()));
                                token_pairs_idx += 1;
                            }

                        } else {
                            token_pairs.push((token, token_slice.to_string()));
                            token_pairs.push((next_token, next_token_slice.to_string()));
                            token_pairs_idx += 2;
                        }
                    }

                } else {
                    token_pairs.push((token, token_slice.to_string()));
                    token_pairs_idx += 1;
                }
            }

            Self {
                token_pairs,
                arg_places: Some(arg_places)
            }
            
        } else {
            loop {
                let token = guarded_unwrap!(guarded_unwrap!(lexer.next(), break), continue);
                let slice = lexer.slice().to_string();
                token_pairs.push((token, slice));
            }

            Self {
                token_pairs,
                arg_places: None
            }
        }
    }

    pub fn iter(
        &'a self,
        args_tokens: Option<Vec<Vec<(Token, &'a str)>>>
    ) -> MacroTokenIterator<'a> {
        if let Some(args_tokens) = args_tokens {
            if let Some(arg_places) = &self.arg_places {
                let args = Some(MacroArgs {
                    args_tokens, arg_places
                });

                return MacroTokenIterator::new(&self.token_pairs, args)
            }
        }

        MacroTokenIterator::new(&self.token_pairs, None)
    }
}

#[derive(Debug, Clone)]
pub struct MacroGroup(HashMap<String, HashMap<usize, Macro>>);

impl<'a> MacroGroup {
    pub fn new() -> Self {
        MacroGroup(HashMap::new())
    }

    fn insert(&mut self, name: &str, body: String, args: Option<HashMap<&'a str, usize>>) {
        let macro_map = self.0.entry(name.into()).or_insert(HashMap::new());
        if let Some(some_args) = &args {
            macro_map.insert(
                some_args.len(),
                Macro::new(body, args)
            );
        } else {
            macro_map.insert(0, Macro::new(body, None));
        };
    }

    pub fn get(&self, macro_name: &str, args_len: usize) -> Option<&Macro> {
        let macro_data = guarded_unwrap!(self.0.get(macro_name), return None);

        macro_data.get(&args_len)
    }
}



