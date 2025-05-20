use crate::string_clip::StringClip;
use super::DerivesToken;
use guarded::guarded_unwrap;
use indexmap::IndexSet;
use std::sync::LazyLock;
use regex::Regex;

const MULTI_LINE_STRING_STRIP_LEFT_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[ \t\f]*\n+").unwrap());

type TokenWithResult<'a, R> = (Option<DerivesToken>, R);

struct DerivesParser<'a> {
    lexer: &'a mut logos::Lexer<'a, DerivesToken>,

    derives: IndexSet<String>,

    nestedness: usize,

    did_advance: bool,
}

impl<'a> DerivesParser<'a> {
    fn new(lexer: &'a mut logos::Lexer<'a, DerivesToken>) -> Self {
        Self {
            lexer,

            derives: IndexSet::new(),

            nestedness: 0,

            did_advance: false,
        }
    }

    fn slice(&self) -> &'a str {
        self.lexer.slice()
    }

    // The `advance` method performs work which would be redundant for:
    // `parse_comment_multi`, `parse_comment_single`, `parse_string_multi_end`.
    // So this core method serves to strip all of it away.
    fn core_advance(&mut self) -> Option<DerivesToken> {
        self.did_advance = true;

        loop {
            match self.lexer.next() {
                Some(Ok(token)) => break Some(token),
                None => return None,
                _ => ()
            }
        }
    }

    fn advance(self: &mut DerivesParser<'a>) -> Option<DerivesToken> {
        let token = guarded_unwrap!(self.core_advance(), return None);

        let token = parse_comment_multi(self, token).unwrap_or(token);
 
        Some(parse_comment_single(self, token).unwrap_or(token))
    }
}

fn parse_comment_multi_end<'a>(
    parser: &mut DerivesParser<'a>, start_equals_amount: usize
) -> Option<DerivesToken> {
    // We keep advancing tokens until we find a closing multiline string
    // token with the same amount of equals signs as the start token.
    loop {
        let token = parser.core_advance()?;

        if let DerivesToken::StringMultiEnd = token {
            let end_token_value = parser.slice();
            let end_equals_amount = end_token_value.clip(1, 1).len();

            if start_equals_amount == end_equals_amount {
                return parser.core_advance()
            }
        }
    }
}

fn parse_comment_multi<'a>(parser: &mut DerivesParser<'a>, token: DerivesToken) -> Option<DerivesToken> {
    if let DerivesToken::CommentMultiStart = token {
        let token_value = parser.slice();
        let start_equals_amount = token_value.clip(3, 1).len();

        return parse_comment_multi_end(parser, start_equals_amount);
    };

    None
}

fn parse_comment_single<'a>(parser: &mut DerivesParser<'a>, token: DerivesToken) -> Option<DerivesToken> {
    if !matches!(token, DerivesToken::CommentSingle) { return None }

    parser.core_advance()
}

fn parse_string_multi_end<'a>(
    parser: &mut DerivesParser<'a>, start_equals_amount: usize
) -> TokenWithResult<'a, String> {
    let mut string_data = String::new();

    // We keep advancing tokens until we find a closing multiline string
    // token with the same amount of equals signs as the start token.
    loop {
        match parser.lexer.next() {
            Some(Ok(token)) => {
                if let DerivesToken::StringMultiEnd = token {
                    let end_token_value = parser.slice();
                    let end_equals_amount = end_token_value.clip(1, 1).len();
        
                    if start_equals_amount == end_equals_amount {
                        return (parser.core_advance(), string_data)
                    }
                }

                string_data += parser.slice();
            },

            Some(Err(_)) => {
                string_data += parser.slice();
            },

            None => return (None, string_data)
        };
    }
}

fn parse_string_multi<'a>(parser: &mut DerivesParser<'a>, token: DerivesToken) -> TokenWithResult<'a, Option<String>> {
    if let DerivesToken::StringMultiStart = token {
        let token_value = parser.slice();
        let start_equals_amount = token_value.clip(1, 1).len();

        let (token, string_data) = parse_string_multi_end(parser, start_equals_amount);

        // Luau strips multiline strings up until the first occurance of a newline character.
        // So we will mimic this behaviour.
        let string_data = MULTI_LINE_STRING_STRIP_LEFT_REGEX.replace(&string_data, "").to_string();
        return (token, Some(string_data))
    };

    (Some(token), None)
}

fn parse_string_single<'a>(parser: &mut DerivesParser<'a>, token: DerivesToken) -> TokenWithResult<'a, Option<String>> {
    match token {
        DerivesToken::StringSingle => {
            let str = parser.slice();
            return (parser.advance(), Some(str.clip(1, 1).to_string()))
        },

        _ => (Some(token), None)
    }
}

fn parse_string<'a>(parser: &mut DerivesParser<'a>, token: DerivesToken) -> TokenWithResult<'a, Option<String>> {
    let parsed = parse_string_single(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_string_multi(parser, token);
    if parsed.1.is_some() { return parsed }

    (Some(token), None)
}

fn parse_string_group<'a>(parser: &mut DerivesParser<'a>, token: DerivesToken) -> Option<DerivesToken> {
    if !matches!(token, DerivesToken::ParensOpen) { return Some(token) }

    let mut nestedness = 0;

    let mut next_token = parser.advance()?;
    loop {
        match next_token {
            DerivesToken::ParensClose => if nestedness == 0 { break } else { nestedness -= 1 },
            DerivesToken::ParensOpen => nestedness += 1,
            _ => {
                if nestedness == 0 {
                    let derive = parse_string(parser, next_token);
                    if let Some(derive_string) = derive.1 {
                        parser.derives.insert(derive_string);

                        next_token = guarded_unwrap!(derive.0, break);

                        loop {
                            match next_token {
                                DerivesToken::ParensClose => if nestedness == 0 { break } else { nestedness -= 1 },
                                DerivesToken::ParensOpen => nestedness += 1,
                                DerivesToken::Comma => break,
                                _ => ()
                            }
                            next_token = parser.advance()?;
                        }
                    }
                }
            }
        }
        next_token = parser.advance()?;
    };

    Some(next_token)
}

fn parse_derive_body<'a>(parser: &mut DerivesParser<'a>, token: DerivesToken) -> Option<DerivesToken> {
    let parsed = parse_string(parser, token);
    if let Some(derive_string) = parsed.1 {
        parser.derives.insert(derive_string);

        return parsed.0
    }

    return parse_string_group(parser, token)
}

fn parse_delimiters<'a>(parser: &mut DerivesParser<'a>, token: DerivesToken)  -> Option<DerivesToken> {
    if matches!(token, DerivesToken::SemiColon | DerivesToken::Comma) {
        let token = guarded_unwrap!(parser.advance(), return None);
        return parse_delimiters(parser, token);
    }

    return Some(token)
}

fn parse_derive_declaration<'a>(parser: &mut DerivesParser<'a>, token: DerivesToken) -> Option<DerivesToken> {
    if !matches!(token, DerivesToken::DeriveDeclaration) { return Some(token) }

    let token = parser.advance()?;

    if parser.nestedness == 0 {
        let token = guarded_unwrap!(parse_derive_body(parser, token), return None);
        parse_delimiters(parser, token)
        
    } else {
        Some(token)
    }
}

fn parse_scope_open<'a>(parser: &mut DerivesParser<'a>, token: DerivesToken) -> Option<DerivesToken> {
    if !matches!(token, DerivesToken::ScopeOpen) { return Some(token) }

    parser.advance()
}

fn parse_scope_close<'a>(parser: &mut DerivesParser<'a>, token: DerivesToken) -> Option<DerivesToken> {
    if !matches!(token, DerivesToken::ScopeOpen) { return Some(token) }

    parser.advance()
}

fn main_loop<'a>(parser: &mut DerivesParser<'a>) -> Option<()> {
    let mut token = guarded_unwrap!(parser.advance(), return None);

    loop {
        parser.did_advance = false;

        token = parse_derive_declaration(parser, token)?;
        token = parse_scope_open(parser, token)?;
        token = parse_scope_close(parser, token)?;

        // Ensures the parser is advanced at least one time per iteration.
        // This prevents infinite loops.
        if !parser.did_advance {
            token = guarded_unwrap!(parser.advance(), break)
        }
    }

    None
}

pub fn parse_rsml_derives<'a>(lexer: &'a mut logos::Lexer<'a, DerivesToken>) -> IndexSet<String> {
    let mut parser = DerivesParser::<'a>::new(lexer);

    main_loop(&mut parser);

    return parser.derives;
}