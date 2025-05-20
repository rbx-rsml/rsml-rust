use std::collections::HashMap;

use guarded::guarded_unwrap;

use crate::string_clip::StringClip;

use super::utils_lexer::UtilsToken;

struct UtilsParser<'a> {
    lexer: &'a mut logos::Lexer<'a, UtilsToken>,
    did_advance: bool,
    utils: HashMap<String, String>
}

impl<'a> UtilsParser<'a> {
    fn new(lexer: &'a mut logos::Lexer<'a, UtilsToken>) -> Self {
        Self {
            lexer,
            did_advance: false,
            utils: HashMap::new()
        }
    }

    // The `advance` method performs work which would be redundant for:
    // `parse_comment_multi`, `parse_comment_single`, `parse_string_multi_end`.
    // So this core method serves to strip all of it away.
    fn core_advance(&mut self) -> Option<UtilsToken> {
        self.did_advance = true;

        loop {
            match self.lexer.next() {
                Some(Ok(token)) => break Some(token),
                None => return None,
                _ => ()
            }
        }
    }

    fn advance(self: &mut UtilsParser<'a>) -> Option<UtilsToken> {
        let token = guarded_unwrap!(self.core_advance(), return None);

        let token = parse_comment_multi(self, token).unwrap_or(token);

        Some(parse_comment_single(self, token).unwrap_or(token))
    }
}

fn parse_comment_multi_end<'a>(
    parser: &mut UtilsParser<'a>, start_equals_amount: usize
) -> Option<UtilsToken> {
    // We keep advancing tokens until we find a closing multiline string
    // token with the same amount of equals signs as the start token.
    loop {
        let token = parser.core_advance()?;

        if let UtilsToken::StringMultiEnd = token {
            let end_token_value = parser.lexer.slice();
            let end_equals_amount = end_token_value.clip(1, 1).len();

            if start_equals_amount == end_equals_amount {
                return parser.core_advance()
            }
        }
    }
}

fn parse_comment_multi<'a>(parser: &mut UtilsParser<'a>, token: UtilsToken) -> Option<UtilsToken> {
    if let UtilsToken::CommentMultiStart = token {
        let token_value = parser.lexer.slice();
        let start_equals_amount = token_value.clip(3, 1).len();

        return parse_comment_multi_end(parser, start_equals_amount);
    };

    None
}

fn parse_comment_single<'a>(parser: &mut UtilsParser<'a>, token: UtilsToken) -> Option<UtilsToken> {
    if !matches!(token, UtilsToken::CommentSingle) { return None }

    parser.core_advance()
}

fn parse_scope_open<'a>(parser: &mut UtilsParser<'a>, token: UtilsToken) -> Option<UtilsToken> {
    if !matches!(token, UtilsToken::ScopeOpen) { return Some(token) }

    parser.advance()
}

fn parse_scope_close<'a>(parser: &mut UtilsParser<'a>, token: UtilsToken) -> Option<UtilsToken> {
    if !matches!(token, UtilsToken::ScopeOpen) { return Some(token) }

    parser.advance()
}


fn parse_util_body<'a>(
    parser: &mut UtilsParser<'a>, token: UtilsToken, util_name: &str
) -> Option<UtilsToken> {
    if !matches!(token, UtilsToken::ScopeOpen) { return Some(token) }

    let mut util_body = String::from(format!(".{util_name} {{"));

    let mut nestedness = 0;

    loop {
        match parser.lexer.next() {
            Some(Ok(token)) => {
                match token {
                    UtilsToken::ScopeOpen => nestedness += 1,
                    UtilsToken::ScopeClose => match nestedness {
                        // End of parsing util.
                        0 => {
                            util_body += "}";

                            parser.utils.insert(util_name.into(), util_body);

                            return parser.advance()
                        }
                        _ => nestedness -= 1
                    },
                    _ => ()
                }

                util_body += parser.lexer.slice()
            },

            Some(Err(_)) => {
                util_body += parser.lexer.slice()
            },

            None => return None
        }
    }
}

fn parse_util_declaration<'a>(parser: &mut UtilsParser<'a>, token: UtilsToken) -> Option<UtilsToken> {
    if !matches!(token, UtilsToken::UtilDeclaration) { return Some(token) }

    let token = parser.advance()?;
    if !matches!(token, UtilsToken::Text) { return Some(token) }
    
    let util_name = parser.lexer.slice();

    let token = parser.advance()?;
    parse_util_body(parser, token, util_name)
}

fn main_loop(mut parser: &mut UtilsParser) -> Option<()> {
    let mut token = guarded_unwrap!(parser.advance(), return None);

    loop {
        parser.did_advance = false;

        token = parse_util_declaration(&mut parser, token)?;
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

pub fn parse_rsml_utils<'a>(lexer: &'a mut logos::Lexer<'a, UtilsToken>) -> HashMap<String, String> {
    let mut parser = UtilsParser::new(lexer);

    main_loop(&mut parser);

    return parser.utils
}