use std::collections::HashMap;

use guarded::guarded_unwrap;

use crate::string_clip::StringClip;

use super::{macros_lexer::MacrosToken, MacroGroup};

type TokenWithResult<'a, R> = (Option<MacrosToken>, R);

struct MacrosParser<'a> {
    lexer: &'a mut logos::Lexer<'a, MacrosToken>,
    did_advance: bool,
    macro_group: &'a mut MacroGroup
}

impl<'a> MacrosParser<'a> {
    fn new(macro_group: &'a mut MacroGroup, lexer: &'a mut logos::Lexer<'a, MacrosToken>) -> Self {
        Self {
            lexer,
            did_advance: false,
            macro_group
        }
    }

    // The `advance` method performs work which would be redundant for:
    // `parse_comment_multi`, `parse_comment_single`, `parse_string_multi_end`.
    // So this core method serves to strip all of it away.
    fn core_advance(&mut self) -> Option<MacrosToken> {
        self.did_advance = true;

        loop {
            match self.lexer.next() {
                Some(Ok(token)) => break Some(token),
                None => return None,
                _ => ()
            }
        }
    }

    fn advance(self: &mut MacrosParser<'a>) -> Option<MacrosToken> {
        let token = guarded_unwrap!(self.core_advance(), return None);

        let token = parse_comment_multi(self, token).unwrap_or(token);

        Some(parse_comment_single(self, token).unwrap_or(token))
    }
}

fn parse_comment_multi_end<'a>(
    parser: &mut MacrosParser<'a>, start_equals_amount: usize
) -> Option<MacrosToken> {
    // We keep advancing tokens until we find a closing multiline string
    // token with the same amount of equals signs as the start token.
    loop {
        let token = parser.core_advance()?;

        if let MacrosToken::StringMultiEnd = token {
            let end_token_value = parser.lexer.slice();
            let end_equals_amount = end_token_value.clip(1, 1).len();

            if start_equals_amount == end_equals_amount {
                return parser.core_advance()
            }
        }
    }
}

fn parse_comment_multi<'a>(parser: &mut MacrosParser<'a>, token: MacrosToken) -> Option<MacrosToken> {
    if let MacrosToken::CommentMultiStart = token {
        let token_value = parser.lexer.slice();
        let start_equals_amount = token_value.clip(3, 1).len();

        return parse_comment_multi_end(parser, start_equals_amount);
    };

    None
}

fn parse_comment_single<'a>(parser: &mut MacrosParser<'a>, token: MacrosToken) -> Option<MacrosToken> {
    if !matches!(token, MacrosToken::CommentSingle) { return None }

    parser.core_advance()
}

fn parse_scope_open<'a>(parser: &mut MacrosParser<'a>, token: MacrosToken) -> Option<MacrosToken> {
    if !matches!(token, MacrosToken::ScopeOpen) { return Some(token) }

    parser.advance()
}

fn parse_scope_close<'a>(parser: &mut MacrosParser<'a>, token: MacrosToken) -> Option<MacrosToken> {
    if !matches!(token, MacrosToken::ScopeOpen) { return Some(token) }

    parser.advance()
}


fn parse_macro_body<'a>(
    parser: &mut MacrosParser<'a>, token: MacrosToken
) -> TokenWithResult<'a, Option<String>> {
    if !matches!(token, MacrosToken::ScopeOpen) { return (Some(token), None) }

    let mut macro_body = String::new();

    let mut nestedness = 0;

    loop {
        match parser.lexer.next() {
            Some(Ok(token)) => {
                match token {
                    MacrosToken::ScopeOpen => nestedness += 1,
                    MacrosToken::ScopeClose => match nestedness {
                        // End of parsing macro.
                        0 => {
                            return (parser.advance(), Some(macro_body))
                        }
                        _ => nestedness -= 1
                    },
                    _ => ()
                }

                macro_body += parser.lexer.slice()
            },

            Some(Err(_)) => {
                macro_body += parser.lexer.slice()
            },

            None => return (None, None)
        }
    }
}

fn parse_macro_args<'a>(parser: &mut MacrosParser<'a>, token: MacrosToken) -> TokenWithResult<'a, Option<HashMap<&'a str, usize>>> {
    if !matches!(token, MacrosToken::ParensOpen) { return (Some(token), None) }

    let mut macro_args = HashMap::new();
    let mut idx = 0;

    loop {
        let token = guarded_unwrap!(parser.advance(), return (None, None));
        match token {
            MacrosToken::Text => {
                macro_args.insert(parser.lexer.slice(), idx);
                idx += 1;
            },

            MacrosToken::ParensClose => return (parser.advance(), Some(macro_args)),

            _ => ()
        }
    }
}

fn parse_macro_declaration<'a>(parser: &mut MacrosParser<'a>, token: MacrosToken) -> Option<MacrosToken> {
    if !matches!(token, MacrosToken::MacroDeclaration) { return Some(token) }

    let token = parser.advance()?;
    if !matches!(token, MacrosToken::Text) { return Some(token) }
    
    let macro_name = parser.lexer.slice();

    let token = parser.advance()?;

    let (token, macro_args) = parse_macro_args(parser, token);
    let token = guarded_unwrap!(token, return None);

    let (token, macro_body) = parse_macro_body(parser, token);

    if let Some(macro_body) = macro_body {
        parser.macro_group.insert(macro_name, macro_body, macro_args);
    }

    token
}

fn main_loop(mut parser: &mut MacrosParser) -> Option<()> {
    let mut token = guarded_unwrap!(parser.advance(), return None);

    loop {
        parser.did_advance = false;

        token = parse_macro_declaration(&mut parser, token)?;
        token = parse_scope_open(&mut parser, token)?;
        token = parse_scope_close(&mut parser, token)?;

        // Ensures the parser is advanced at least one time per iteration.
        // This prevents infinite loops.
        if !parser.did_advance {
            token = guarded_unwrap!(parser.advance(), break)
        }
    }

    None
}

pub fn parse_rsml_macros<'a>(macro_group: &'a mut MacroGroup, lexer: &'a mut logos::Lexer<'a, MacrosToken>) {
    let mut parser = MacrosParser::new(macro_group, lexer);

    main_loop(&mut parser);
}