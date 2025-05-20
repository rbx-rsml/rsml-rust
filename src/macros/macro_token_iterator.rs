use std::{collections::HashMap, fmt::Debug};

use crate::Token;

use super::TokenPair;

#[derive(Debug)]
pub struct MacroTokenIterator<'a> {
    token_pairs: &'a Vec<(Token, String)>,
    tokens_len: usize,

    args: Option<MacroArgs<'a>>,
    current_arg_idx: Option<usize>,
    current_arg_token_idx: usize,
    current_args_tokens_len: usize,

    current_position: usize,
    current_slice: Option<&'a str>,
}

#[derive(Debug)]
pub struct MacroArgs<'a> {
    pub args_tokens: Vec<Vec<(Token, &'a str)>>,

    // Left is the place, right is the arg idx.
    pub arg_places: &'a HashMap<usize, usize>,
}


impl<'a> MacroTokenIterator<'a> {
    pub fn new(
        token_pairs: &'a Vec<TokenPair>,
        args: Option<MacroArgs<'a>>,
    ) -> Self {
        let tokens_len = token_pairs.len();

        Self {
            token_pairs,
            tokens_len: tokens_len,

            args,
            current_arg_idx: None,
            current_arg_token_idx: 0,
            current_args_tokens_len: 0,

            current_position: 0,
            current_slice: None
        }
    }

    pub fn slice(&self) -> &'a str {
        self.current_slice.unwrap()
    }
}

impl<'a> Iterator for MacroTokenIterator<'a> {
    type Item = Result<Token, ()>;

    fn next(&mut self) -> Option<Result<Token, ()>> {
        let current_position = self.current_position;

        if let Some(current_arg_idx) = &self.current_arg_idx {
            let current_arg_token_idx = self.current_arg_token_idx + 1;

            if current_arg_token_idx != self.current_args_tokens_len {
                let (token, slice) =
                self.args.as_ref().unwrap().args_tokens.get(*current_arg_idx).unwrap()[current_arg_token_idx];

                self.current_arg_token_idx = current_arg_token_idx;
                self.current_slice = Some(slice);

                return Some(Ok(token))

            } else {
                self.current_arg_idx = None;
            }


        } else if let Some(args) = self.args.as_ref() {
            if let Some(current_arg_idx) = args.arg_places.get(&current_position) {
                let tokens = args.args_tokens.get(*current_arg_idx).unwrap();

                self.current_arg_idx = Some(*current_arg_idx);
                self.current_arg_token_idx = 0;
                self.current_args_tokens_len = tokens.len();

                let (token, slice) = tokens[0];

                self.current_slice = Some(slice);

                return Some(Ok(token))
            }
        };

        if self.tokens_len == current_position {
            None
        } else {
            let (token, slice) = &self.token_pairs[current_position];

            self.current_position = current_position + 1;
            self.current_slice = Some(slice);

            Some(Ok(*token))
        }
    }
}