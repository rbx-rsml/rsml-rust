use crate::string_clip::StringClip;
use crate::lexer::Token;
use std::{ops::Deref, ptr, sync::LazyLock};
use num_traits::Num;
use rbx_types::{UDim, Variant};
use regex::Regex;
use crate::guarded_unwrap;

mod datatype_operations;
use datatype_operations::datatype_operation;

mod tuple;
use tuple::Tuple;

mod tree_node;
pub use tree_node::TreeNode;

mod selector;
use selector::Selector;

mod datatype;
use datatype::Datatype;

mod operator;
use operator::Operator;

mod colors {
    include!(concat!(env!("OUT_DIR"), "/colors.rs"));
}
use colors::{TAILWIND_COLORS, BRICK_COLORS, CSS_COLORS};

static MULTI_LINE_STRING_STRIP_LEFT_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[ \t\f]*\n+").unwrap());

type TokenWithResult<'a, R> = (Option<Token<'a>>, R);

struct Parser<'a> {
    lexer: &'a mut logos::Lexer<'a, Token<'a>>,

    tree_nodes: Vec<TreeNode>,
    current_tree_node: usize,

    tuples: Vec<Tuple>,

    did_advance: bool,
}

impl<'a> Parser<'a> {
    fn new(lexer: &'a mut logos::Lexer<'a, Token<'a>>) -> Self {
        Self {
            lexer,

            tree_nodes: vec![],
            current_tree_node: 0,

            tuples: vec![],

            did_advance: false,
        }
    }

    // The `advance` method performs work which would be redundant for:
    // `parse_comment_multi`, `parse_comment_single`, `parse_string_multi_end`.
    // So this core method serves to strip all of it away.
    fn core_advance(&mut self) -> Option<Token<'a>> {
        self.did_advance = true;

        loop {
            match self.lexer.next() {
                Some(Ok(token)) => break Some(token),
                None => return None,
                _ => ()
            }
        }
    }

    fn advance(self: &mut Parser<'a>) -> Option<Token<'a>> {
        let token = guarded_unwrap!(self.core_advance(), return None);

        let token = parse_comment_multi(self, token).unwrap_or(token);

        Some(parse_comment_single(self, token).unwrap_or(token))
    }

    // create remove tuple for when a tuple is coerced into a datatype and thusly
    // never used again.
    fn push_tuple(self: &mut Parser<'a>, tuple: Tuple) -> usize {
        let tuples = &mut self.tuples;

        let new_tuple_idx = tuples.len();
        tuples.push(tuple);

        new_tuple_idx
    }

    fn get_tuple(self: &mut Parser<'a>, tuple_idx: usize) -> Option<&Tuple> {
        self.tuples.get(tuple_idx)
    }

    fn get_tuple_mut<'b>(self: &'b mut Parser<'a>, tuple_idx: usize) -> Option<&'b mut Tuple> {
        self.tuples.get_mut(tuple_idx)
    }
}

fn parse_comment_multi_end<'a>(parser: &mut Parser<'a>, start_equals_amount: usize) -> Option<Token<'a>> {
    // We keep advancing tokens until we find a closing multiline string
    // token with the same amount of equals signs as the start token.
    loop {
        let token = parser.core_advance()?;

        if let Token::StringMultiEnd(end_token_value) = token {
            let end_equals_amount = end_token_value.clip(1, 1).len();

            if start_equals_amount == end_equals_amount {
                return parser.core_advance()
            }
        }
    }
}

fn parse_comment_multi<'a>(parser: &mut Parser<'a>, token: Token) -> Option<Token<'a>> {
    if let Token::CommentMultiStart(token_value) = token {
        let start_equals_amount = token_value.clip(3, 1).len();

        return parse_comment_multi_end(parser, start_equals_amount);
    };

    None
}

fn parse_comment_single<'a>(parser: &mut Parser<'a>, token: Token) -> Option<Token<'a>> {
    if !matches!(token, Token::CommentSingle(_)) { return None }

    parser.core_advance()
}

fn parse_scope_open<'a>(parser: &mut Parser<'a>, token: Token<'a>, selector: Option<String>) -> Option<Token<'a>> {
    if !matches!(token, Token::ScopeOpen) { return Some(token) }

    let new_tree_node_idx = parser.tree_nodes.len();

    let previous_tree_node = &mut parser.tree_nodes[parser.current_tree_node];
    previous_tree_node.rules.push(new_tree_node_idx);

    let current_tree_node = TreeNode::new(parser.current_tree_node, selector);
    parser.tree_nodes.push(current_tree_node);

    parser.current_tree_node = new_tree_node_idx;

    parser.advance()
}

fn parse_scope_delimiter<'a>(
    parser: &mut Parser<'a>,
    token: Token<'a>,
    delim_token: Option<Token<'a>>
) -> TokenWithResult<'a, Option<Token<'a>>> {
    if matches!(token, Token::Comma) {
        let next_token = guarded_unwrap!(parser.advance(), return (None, delim_token));
        return parse_scope_delimiter(parser, next_token, Some(token))

    } else {
        return (Some(token), delim_token)
    }
}

fn parse_scope_selector_start<'a>(parser: &mut Parser<'a>, token: Token<'a>) -> Option<Token<'a>> {
    // The `Token::Text(_)` case is handled in `parse_property` and `parse_attribute`.
    if !matches!(token, 
        Token::NameIdentifier | Token::PsuedoIdentifier | Token::StateOrEnumIdentifier |
        Token::TagOrEnumIdentifier | Token::ScopeToDescendants | Token::ScopeToChildren
    ) { return Some(token) }

    let selector = parser.lexer.slice();
    let selector_token = token;

    let token = parser.advance()?;
    return parse_scope_selector(parser, token, Selector::new(selector, selector_token));
}

fn parse_scope_selector<'a>(
    parser: &mut Parser<'a>, mut token: Token<'a>, mut selector: Selector<'a>
) -> Option<Token<'a>> {
    loop {
        // advances the parser until no delimiter token is found.
        let parsed_delimiter = parse_scope_delimiter(parser, token, None);
        token = guarded_unwrap!(parsed_delimiter.0, return None);

        if matches!(token, 
            Token::NameIdentifier | Token::PsuedoIdentifier | Token::StateOrEnumIdentifier |
            Token::TagOrEnumIdentifier | Token::ScopeToDescendants | Token::ScopeToChildren |
            Token::Text(_)
        ) {
            // Appending the comma token here ensures selectors can't end with delimiters.
            if let Some(delim_token) = parsed_delimiter.1 {
                selector.append(",", delim_token);
            }
            
            selector.append(parser.lexer.slice(), token);

            token = parser.advance()?;

        } else { break }
    };

    return parse_scope_open(parser, token, Some(selector.content))
}


fn parse_scope_close<'a>(parser: &mut Parser<'a>, token: Token<'a>) -> Option<Token<'a>> {
    if !matches!(token, Token::ScopeClose) { return Some(token) }

    let current_tree_node = &parser.tree_nodes[parser.current_tree_node];
    parser.current_tree_node = current_tree_node.parent;

    return parser.advance()
}

fn parse_string_multi_end<'a>(
    parser: &mut Parser<'a>, start_equals_amount: usize, token_history: &mut Vec<&'a str>
) -> Option<Token<'a>> {
    // We keep advancing tokens until we find a closing multiline string
    // token with the same amount of equals signs as the start token.
    loop {
        match parser.lexer.next() {
            Some(Ok(token)) => {
                if let Token::StringMultiEnd(end_token_value) = token {
                    let end_equals_amount = end_token_value.clip(1, 1).len();
        
                    if start_equals_amount == end_equals_amount {
                        return parser.core_advance()
                    }
                }

                token_history.push(parser.lexer.slice());
            },

            Some(Err(_)) => {
               token_history.push(parser.lexer.slice())
            },

            None => ()
        };
    }
}

fn parse_string_multi_datatype<'a>(parser: &mut Parser<'a>, token: Token<'a>) -> TokenWithResult<'a, Option<Datatype>> {
    if let Token::StringMultiStart(token_value) = token {
        let start_equals_amount = token_value.clip(1, 1).len();

        let mut token_history = vec![];
        let token = parse_string_multi_end(parser, start_equals_amount, &mut token_history);

        // Joins the string tokens together ignoring the opening and closing tokens.
        let mut str = "".to_string();
        for token in token_history {
            str += &token
        }

        // Luau strips multiline strings up until the first occurance of a newline character.
        // So we will mimic this behaviour.
        str = MULTI_LINE_STRING_STRIP_LEFT_REGEX.replace(&str, "").to_string();
        return (token, Some(Datatype::Variant(Variant::String(str))))
    };

    (Some(token), None)
}

fn parse_string_single_datatype<'a>(parser: &mut Parser<'a>, token: Token<'a>) -> TokenWithResult<'a, Option<Datatype>> {
    match token {
        Token::StringSingle(str) => {
            let token = parser.advance();
            let datatype = Some(Datatype::Variant(Variant::String(str.clip(1, 1).to_string())));
            return (token, datatype)
        },

        Token::RobloxAsset(str) => {
            let token = parser.advance();
            let datatype = Some(Datatype::Variant(Variant::String(str.to_string())));
            return (token, datatype)
        },

        _ => (Some(token), None)
    }
}

fn parse_string_datatype<'a>(parser: &mut Parser<'a>, token: Token<'a>) -> TokenWithResult<'a, Option<Datatype>> {
    let parsed = parse_string_single_datatype(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_string_multi_datatype(parser, token);
    if parsed.1.is_some() { return parsed }

    (Some(token), None)
}

fn parse_number_offset<'a>(parser: &mut Parser<'a>, token: Token<'a>, num_str: &str) -> TokenWithResult<'a, Option<Datatype>> {
    if !matches!(token, Token::Offset) { return (Some(token), None) }

    let token = parser.advance();
    let datatype = Some(Datatype::Variant(Variant::UDim(UDim::new(0.0, num_str.parse::<i32>().unwrap()))));

    return (token, datatype)
}

fn parse_number_scale<'a>(parser: &mut Parser<'a>, token: Token<'a>, num_str: &str) -> TokenWithResult<'a, Option<Datatype>> {
    if !matches!(token, Token::ScaleOrOpMod) { return (Some(token), None) }

    let token = parser.advance();
    let datatype = Some(Datatype::Variant(Variant::UDim(UDim::new(num_str.parse::<f32>().unwrap() / 100.0, 0))));

    return (token, datatype)
}

fn parse_number_datatype<'a>(parser: &mut Parser<'a>, token: Token<'a>) -> TokenWithResult<'a, Option<Datatype>> {
    if let Token::Number(token_value) = token {
        let token = parser.advance();

        if let Some(token) = token {
            let parsed = parse_number_offset(parser, token, token_value);
            if parsed.1.is_some() { return parsed }

            let parsed = parse_number_scale(parser, token, token_value);
            if parsed.1.is_some() { return parsed }
        }

        let datatype = Some(Datatype::Variant(Variant::Float32(token_value.parse::<f32>().unwrap())));

        return (token, datatype)
    }

    (Some(token), None)
}

fn parse_operator_datatype<'a>(parser: &mut Parser<'a>, token: Token<'a>) -> TokenWithResult<'a, Option<Datatype>> {
    match token {
        Token::OpPow => {
            return (parser.advance(), Some(Datatype::Operator(Operator::Pow)))
        },
        Token::OpDiv => {
            (parser.advance(), Some(Datatype::Operator(Operator::Div)))
        },
        Token::OpMod | Token::ScaleOrOpMod => {
            (parser.advance(), Some(Datatype::Operator(Operator::Mod)))
        },
        Token::OpMult => {
            (parser.advance(), Some(Datatype::Operator(Operator::Mult)))
        },
        Token::OpAdd => {
            (parser.advance(), Some(Datatype::Operator(Operator::Add)))
        },
        Token::OpSub => {
            (parser.advance(), Some(Datatype::Operator(Operator::Sub)))
        },
        _ => (Some(token), None)
    }
}

fn parse_enum_tokens<'a>(token_history: &mut Vec<Token<'a>>) -> Option<Datatype> {
    let token_history_len = token_history.len();

    let mut full_enum = "Enum".to_string();

    let mut first_stop_idx = 0;
    for idx in 0..=token_history_len {
        let token = &token_history[idx];

        if let Token::Text(text)  = token {
            full_enum += &format!(".{}", text);
            first_stop_idx = idx;
            break
        }
    };

    for idx in (first_stop_idx + 1)..=token_history_len {
        let token = &token_history[idx];

        if let Token::Text(text) = token {
            full_enum += &format!(".{}", text);
            break
        }
    };

    return Some(Datatype::Variant(Variant::String(full_enum)))
}

fn parse_full_enum<'a>(
    parser: &mut Parser<'a>, token: Token<'a>, token_history: &mut Vec<Token<'a>>
) -> TokenWithResult<'a, Option<Datatype>> {
    if matches!(token, 
        Token::TagOrEnumIdentifier | Token::StateOrEnumIdentifier | Token::Text(_)
    ) {
        let token = parser.advance();

        if let Some(token) = token {
            token_history.push(token);

            let parsed = parse_full_enum(parser, token, token_history);
            if parsed.1.is_some() { return parsed }
        }
    }

    if token_history.len() == 0 { return (Some(token), None) }

    return (Some(token), parse_enum_tokens(token_history))
}


fn parse_enum_keyword<'a>(parser: &mut Parser<'a>, token: Token<'a>) -> TokenWithResult<'a, Option<Datatype>> {
    if !matches!(token, Token::EnumKeyword) { return (Some(token), None) }

    let token = guarded_unwrap!(parser.advance(), return (Some(token), None) );

    let mut token_history = vec![];
    parse_full_enum(parser, token, &mut token_history)
}

fn parse_enum_shorthand<'a>(parser: &mut Parser<'a>, token: Token<'a>, key: Option<&str>) -> TokenWithResult<'a, Option<Datatype>> {
    let key = guarded_unwrap!(key, return (Some(token), None));

    if !matches!(token, Token::StateOrEnumIdentifier) { return (Some(token), None) }

    let token = guarded_unwrap!(parser.advance(), return (Some(token), None));

    let enum_item = if let Token::Text(text) = token { text } else { return (Some(token), None) };

    // TODO: convert this to its enum member number value (instead of a string) using an api dump.
    let datatype = Some(Datatype::Variant(Variant::String(format!("Enum.{}.{}", key, enum_item))));
    return (parser.advance(), datatype)
}

fn parse_enum_datatype<'a>(parser: &mut Parser<'a>, token: Token<'a>, key: Option<&str>) -> TokenWithResult<'a, Option<Datatype>> {
    let parsed = parse_enum_keyword(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_enum_shorthand(parser, token, key);
    if parsed.1.is_some() { return parsed }

    (Some(token), None)
}

fn parse_boolean_datatype<'a>(parser: &mut Parser<'a>, token: Token<'a>) -> TokenWithResult<'a, Option<Datatype>> {
    match token {
        Token::BoolTrue => {
            (parser.advance(), Some(Datatype::Variant(Variant::Bool(true))))
        },
        Token::BoolFalse => {
            (parser.advance(), Some(Datatype::Variant(Variant::Bool(false))))
        },
        _ => (Some(token), None)
    }
}

#[allow(warnings)]
fn parse_predefined_tailwind_color<'a>(parser: &mut Parser<'a>, token: Token<'a>) -> TokenWithResult<'a, Option<Datatype>> {
    if let Token::ColorTailwind(color) = token {
        let datatype = TAILWIND_COLORS.get(color)
            .and_then(|color| Some(Datatype::Variant(Variant::Color3(**color.deref()))));

        return (parser.advance(), datatype)
    }

    (Some(token), None)
}

#[allow(warnings)]
fn parse_predefined_css_color<'a>(parser: &mut Parser<'a>, token: Token<'a>) -> TokenWithResult<'a, Option<Datatype>> {
    if let Token::ColorCss(color) = token {
        let datatype = CSS_COLORS.get(color)
            .and_then(|color| Some(Datatype::Variant(Variant::Color3(**color.deref()))));

        return (parser.advance(), datatype)
    }

    (Some(token), None)
}

#[allow(warnings)]
fn parse_predefined_brick_color<'a>(parser: &mut Parser<'a>, token: Token<'a>) -> TokenWithResult<'a, Option<Datatype>> {
    if let Token::ColorBrick(color) = token {
        let datatype = BRICK_COLORS.get(color)
            .and_then(|color| Some(Datatype::Variant(Variant::Color3(**color.deref()))));

        return (parser.advance(), datatype)
    }

    (Some(token), None)
}

fn parse_predefined_color_datatype<'a>(parser: &mut Parser<'a>, token: Token<'a>) -> TokenWithResult<'a, Option<Datatype>> {
    let parsed = parse_predefined_tailwind_color(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_predefined_css_color(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_predefined_brick_color(parser, token);
    if parsed.1.is_some() { return parsed }

    return (Some(token), None)
}

fn parse_attribute_name_datatype<'a>(parser: &mut Parser<'a>, token: Token<'a>) -> TokenWithResult<'a, Option<Datatype>> {
    if !matches!(token, Token::AttributeIdentifier) { return (Some(token), None) }

    let token = guarded_unwrap!(parser.advance(), return (Some(token), None));

    if let Token::Text(text) = token {
        return (parser.advance(), Some(Datatype::Variant(Variant::String(format!("${}", text)))))
    }

    (Some(token), None)
}

fn parse_datatype<'a>(
    parser: &mut Parser<'a>, token: Token<'a>, key: Option<&str>
) -> TokenWithResult<'a, Option<Datatype>> {
    if let (Some(token), Some(current_tuple_idx)) = parse_tuple_name(parser, token, None, None) {
        let current_tuple = parser.get_tuple(current_tuple_idx).unwrap();

        return (Some(token), Some(current_tuple.coerce_to_datatype()))
    };
  
    let parsed = parse_string_datatype(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_number_datatype(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_operator_datatype(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_enum_datatype(parser, token, key);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_predefined_color_datatype(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_attribute_name_datatype(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_boolean_datatype(parser, token);
    if parsed.1.is_some() { return parsed }

    (Some(token), None)
}

fn find_operators_in_datatypes<'a>(
    datatypes: &Vec<Datatype>,
    operator_group: &'a [(Operator, fn(f32, f32) -> f32, fn(i32, i32) -> i32)]
) -> Vec<(usize, &'a Operator, fn(f32, f32) -> f32, fn(i32, i32) -> i32)> {
    let mut indexes: Vec<(usize, &'a Operator, fn(f32, f32) -> f32, fn(i32, i32) -> i32)> = vec![];

    'datatype_loop: for (idx, datatype) in datatypes.iter().enumerate() {
        if let Datatype::Operator(datatype_operator) = datatype  {
            for (operator, operation_fn_f32, operation_fn_i32) in operator_group {
                if operator == datatype_operator {
                    indexes.push((idx, operator, *operation_fn_f32, *operation_fn_i32));
                    continue 'datatype_loop;
                }
            }
        }
    }

    return indexes
}

fn will_divide_by_zero<T: Num>(a: T, b: T) -> bool {
    a.is_zero() || b.is_zero()
}

fn add<T: Num>(a: T, b: T) -> T { a + b }
fn sub<T: Num>(a: T, b: T) -> T { a - b }

fn mult<T: Num>(a: T, b: T) -> T { a * b }
fn div<T: Num + Copy + std::fmt::Debug>(a: T, b: T) -> T {
    if will_divide_by_zero(a, b) { return a }
    a / b
}

fn pow_f32(a: f32, b: f32) -> f32 { a.powf(b) }
fn pow_i32(a: i32, b: i32) -> i32 { a.pow(b as u32) }

fn mod_<T: Num>(a: T, b: T) -> T { a % b }

static ORDERED_OPERATORS: &[&[(Operator, fn(f32, f32) -> f32, fn(i32, i32) -> i32)]] = &[
    &[(Operator::Pow, pow_f32, pow_i32)],
    &[
        (Operator::Div, div::<f32>, div::<i32>),
        (Operator::Mod, mod_::<f32>, mod_::<i32>),
        (Operator::Mult, mult::<f32>, mult::<i32>),
    ],
    &[
        (Operator::Add, add::<f32>, add::<i32>),
        (Operator::Sub, sub::<f32>, sub::<i32>),
    ],
];

fn solve_datatype_group(mut datatypes: Vec<Datatype>) -> Datatype {
    for operator_group in ORDERED_OPERATORS {
        let occurrences = find_operators_in_datatypes(&datatypes, operator_group);
        let mut occurrence_idx_offset = 0;

        for (
            mut occurrence_idx, operator, operation_fn_f32, operation_fn_i32
        ) in occurrences {
            occurrence_idx -= occurrence_idx_offset;

            let right_idx = occurrence_idx + 1;
            if right_idx > datatypes.len() { continue; }

            let right = datatypes.remove(right_idx);
            occurrence_idx_offset += 1;

            let (left, left_is_none) = {
                if occurrence_idx == 0 {
                    (Datatype::Variant(Variant::Float32(0.0)), true)

                } else {
                    let left_idx = occurrence_idx - 1;
                    occurrence_idx_offset += 1;
                    let left = datatypes.remove(left_idx);
                    if matches!(left, Datatype::Empty) {
                        (Datatype::Variant(Variant::Float32(0.0)), false)
                    } else {
                        (left, false)
                    }
                }
            };

            let solved_datatype = datatype_operation(
                &left, &right, operator, &operation_fn_f32, &operation_fn_i32
            );
            datatypes[if left_is_none { occurrence_idx } else { occurrence_idx - 1 }] = solved_datatype.unwrap_or(left);
        }
    };

    return datatypes[0].clone()
}

fn resolve_datatype_group<'a>(datatypes: Option<Vec<Datatype>>) -> Option<Datatype> {
    if let Some(datatypes) = datatypes {
        if datatypes.len() == 1 {
            return Some(datatypes[0].clone())
        }

        return Some(solve_datatype_group(datatypes))
    }
    None
}

fn datatypes_ensure_exists_then_insert(datatypes: Option<Vec<Datatype>>, to_insert: Datatype) -> Vec<Datatype> {
    if let Some(mut datatypes) = datatypes {
        datatypes.push(to_insert);
        datatypes
    } else {
        let datatypes = vec![to_insert];
        datatypes
    }
}

fn parse_datatype_group<'a>(
    parser: &mut Parser<'a>, token: Token<'a>, key: Option<&str>,
    mut datatypes: Option<Vec<Datatype>>, mut pending_operator: Option<Operator>
) -> TokenWithResult<'a, Option<Datatype>> {
    let (token, datatype) = parse_datatype(parser, token, key);

    if let Some(datatype) = datatype {
        // If the datatype is an operator then we need to postpone adding it
        // to the datatypes group so we can atomise it with the next datatype
        // if it is an operator.
        if !matches!(datatype, Datatype::Operator(_)) {
             if let Some(some_pending_operator) = pending_operator {
                // We can add our pending operator to the datatypes table
                // since it has no other operator to atomise with.
                let mut datatypes_exists = datatypes_ensure_exists_then_insert(
                    datatypes, 
                    Datatype::Operator(some_pending_operator)
                );
                pending_operator = None;
                datatypes_exists.push(datatype.clone());
                datatypes = Some(datatypes_exists);
                
            } else {
                datatypes = Some(datatypes_ensure_exists_then_insert(datatypes, datatype.clone()))
            }
        }

        if let Some(some_token) = token {
            if matches!(some_token, Token::ParensClose | Token::ScopeClose | Token::Comma | Token::SemiColon) {
                return (token, resolve_datatype_group(datatypes));

            } else {
                if let Datatype::Operator(operator) = datatype {
                    // Since our datatype was an operator we need to mark it as pending,
                    // atomising with the existing pending operator if it exists.
                    if let Some(some_pending_operator) = pending_operator {
                        pending_operator = Some(some_pending_operator.combine_with(&operator))
                    } else {
                        pending_operator = Some(operator)
                    }
                }

                return parse_datatype_group(parser, some_token, key, datatypes, pending_operator);
            }
        } else {
            return (token, resolve_datatype_group(datatypes));
        }

    } else {
        return (token, resolve_datatype_group(datatypes))
    }
}

fn parse_tuple_close<'a>(
    parser: &mut Parser<'a>, token: Token<'a>,
    current_tuple_idx: usize, root_tuple_idx: usize
) -> Option<Token<'a>> {
    if !matches!(token, Token::ParensClose) { return None }

    let token = parser.advance();

    let current_tuple = parser.get_tuple(current_tuple_idx).unwrap();
    let parent_tuple_idx = current_tuple.parent_idx;

    if let Some(some_parent_tuple_idx) = parent_tuple_idx {
        let datatype = current_tuple.coerce_to_datatype();
        parser.get_tuple_mut(some_parent_tuple_idx).unwrap().data.push(datatype);

        if let Some(token) = token {
            let parsed = parse_tuple_delimiter(parser, token, some_parent_tuple_idx, root_tuple_idx);
            if parsed.is_some() { return parsed }

            let parsed = parse_tuple_close(parser, token, some_parent_tuple_idx, root_tuple_idx);
            if parsed.is_some() { return parsed }

            let parsed = parse_tuple_name(
                parser, token, parent_tuple_idx, Some(root_tuple_idx)
            );
            if parsed.1.is_some() { return parsed.0 }
        }
    };

    token
}

fn parse_tuple_datatype<'a>(
    parser: &mut Parser<'a>, token: Token<'a>,
    current_tuple_idx: usize, root_tuple_idx: usize
) -> Option<Token<'a>> {
    let (token, datatype) = parse_datatype_group(
        parser, token, None, None, None
    );

    let datatype = guarded_unwrap!(datatype, return None);

    let current_tuple = parser.get_tuple_mut(current_tuple_idx).unwrap();
    current_tuple.data.push(datatype);

    if let Some(token) = token {
        let parsed = parse_tuple_delimiter(parser, token, current_tuple_idx, root_tuple_idx);
        if parsed.is_some() { return parsed }

        let parsed = parse_tuple_close(parser, token, current_tuple_idx, root_tuple_idx);
        if parsed.is_some() { return parsed }
        
        let parsed = parse_tuple_name(
            parser, token, Some(current_tuple_idx), Some(root_tuple_idx)
        );
        if parsed.1.is_some() { return parsed.0 }
    }

    token
}

fn parse_tuple_delimiter<'a>(
    parser: &mut Parser<'a>, token: Token<'a>,
    current_tuple_idx: usize, root_tuple_idx: usize
) -> Option<Token<'a>> {
    if !matches!(token, Token::SemiColon | Token::Comma) { return None }

    let token = parser.advance();

    if let Some(token) = token {
        // Handles cases of multiple delimiter tokens next to each other.
        let parsed = parse_tuple_delimiter(parser, token, current_tuple_idx, root_tuple_idx);
        if parsed.is_some() { return parsed }
        
        let parsed = parse_tuple_close(parser, token, current_tuple_idx, root_tuple_idx);
        if parsed.is_some() { return parsed }

        let parsed = parse_tuple_name(
            parser, token, Some(current_tuple_idx), Some(root_tuple_idx)
        );
        if parsed.1.is_some() { return parsed.0 }

        let parsed = parse_tuple_datatype(parser, token, current_tuple_idx, root_tuple_idx);
        if parsed.is_some() { return parsed }
    }

    token
}

fn parse_tuple_open<'a>(
    parser: &mut Parser<'a>, token: Token<'a>, tuple_name: Option<String>,
    parent_tuple_idx: Option<usize>, root_tuple_idx: Option<usize>
) -> TokenWithResult<'a, Option<usize>> {
    if !matches!(token, Token::ParensOpen) { return (None, None) };

    let current_tuple_idx = parser.push_tuple(
        Tuple::new(tuple_name, parent_tuple_idx)
    );

    let token = guarded_unwrap!(parser.advance(), return (Some(token), Some(current_tuple_idx)));

    let root_tuple_idx = root_tuple_idx.unwrap_or(current_tuple_idx);

    let parsed = parse_tuple_delimiter(parser, token, current_tuple_idx, root_tuple_idx);
    if parsed.is_some() { return (parsed, Some(current_tuple_idx)) }

    let parsed = parse_tuple_close(parser, token, current_tuple_idx, root_tuple_idx);
    if parsed.is_some() { return (parsed, Some(current_tuple_idx)) }

    let parsed = parse_tuple_name(
        parser, token, Some(current_tuple_idx), Some(root_tuple_idx)
    );
    if parsed.0.is_some() { return (parsed.0, Some(current_tuple_idx)) }

    let parsed = parse_tuple_datatype(parser, token, current_tuple_idx, root_tuple_idx);
    if parsed.is_some() { return (parsed, Some(current_tuple_idx)) }

    (Some(token), Some(current_tuple_idx))
}

fn parse_tuple_name<'a>(
    parser: &mut Parser<'a>, token: Token<'a>,
    parent_tuple_idx: Option<usize>, root_tuple_idx: Option<usize>
) -> TokenWithResult<'a, Option<usize>> {
    if let Token::Text(tuple_name) = token {
        let token = guarded_unwrap!(parser.advance(), return (Some(token), None));

        return parse_tuple_open(parser, token, Some(tuple_name.to_string()), parent_tuple_idx, root_tuple_idx)

    } else {
        return parse_tuple_open(parser, token, None, parent_tuple_idx, root_tuple_idx)
    }
}

fn parse_delimiters<'a>(parser: &mut Parser<'a>, token: Token<'a>)  -> Option<Token<'a>> {
    if matches!(token, Token::SemiColon | Token::Comma) {
        let token = guarded_unwrap!(parser.advance(), return None);
        return parse_delimiters(parser, token);
    }

    return Some(token)
}

fn parse_property<'a>(parser: &mut Parser<'a>, mut token: Token<'a>) -> Option<Token<'a>> {
    if let Token::Text(property_name) = token {
        let selector = parser.lexer.slice();
        let selector_token = token;

        let next_token = guarded_unwrap!(parser.advance(), return Some(token));

        if !matches!(next_token, Token::Equals) {
            return parse_scope_selector(parser, next_token, Selector::new(selector, selector_token))
        };

        token = guarded_unwrap!(parser.advance(), return Some(next_token));

        let (token, datatype) = parse_datatype_group(
            parser, token, Some(property_name), None, None
        );
        let variant = datatype.and_then(|d| d.coerce_to_variant());

        if let Some(variant) = variant {
            let current_tree_node = parser.tree_nodes.get_mut(parser.current_tree_node).unwrap();
            current_tree_node.properties.insert(property_name.to_string(), variant);
        }

        let token = guarded_unwrap!(token, return None);
        return parse_delimiters(parser, token);
    }

    Some(token)
}

fn parse_attribute<'a>(parser: &mut Parser<'a>, mut token: Token<'a>) -> Option<Token<'a>> {
    if !matches!(token, Token::AttributeIdentifier) { return Some(token) };

    token = guarded_unwrap!(parser.advance(), return Some(token));

    if let Token::Text(attribute_name) = token {
        let selector = parser.lexer.slice();
        let selector_token = token;
        
        let next_token = guarded_unwrap!(parser.advance(), return Some(token));
        
        if !matches!(next_token, Token::Equals) {
            return parse_scope_selector(parser, next_token, Selector::new(selector, selector_token))
        }
        

        token = guarded_unwrap!(parser.advance(), return Some(next_token));

        let (token, datatype) = parse_datatype_group(
            parser, token, Some(attribute_name), None, None
        );
        let variant = datatype.and_then(|d| d.coerce_to_variant());

        if let Some(variant) = variant {
            let current_tree_node = parser.tree_nodes.get_mut(parser.current_tree_node).unwrap();
            current_tree_node.attributes.insert(attribute_name.to_string(), variant);
        }

        let token = guarded_unwrap!(token, return None);
        return parse_delimiters(parser, token)
    }

    Some(token)
}

fn parse_priority_declaration<'a>(parser: &mut Parser<'a>, token: Token<'a>) -> Option<Token<'a>> {
    if !matches!(token, Token::PriorityDeclaration) { return Some(token) }

    let token = parser.advance()?;

    let (token, datatype) = parse_datatype_group(parser, token, None, None, None);

    if let Some(Datatype::Variant(Variant::Float32(float32))) = datatype {
        let current_tree_node = &mut parser.tree_nodes[parser.current_tree_node];
        current_tree_node.priority = Some(float32 as i32);
    }

    let token = guarded_unwrap!(token, return None);
    return parse_delimiters(parser, token)
}

fn parse_name_declaration<'a>(parser: &mut Parser<'a>, token: Token<'a>) -> Option<Token<'a>> {
    if !matches!(token, Token::NameDeclaration) { return Some(token) }

    let token = parser.advance()?;

    let (token, name) = parse_string_datatype(parser, token);

    if let Some(Datatype::Variant(Variant::String(name))) = name {
        let current_tree_node = &mut parser.tree_nodes[parser.current_tree_node];
        current_tree_node.name = Some(name);
    }

    let token = guarded_unwrap!(token, return None);
    return parse_delimiters(parser, token)
}

fn parse_derive_declaration<'a>(parser: &mut Parser<'a>, token: Token<'a>) -> Option<Token<'a>> {
    if !matches!(token, Token::DeriveDeclaration) { return Some(token) }

    let token = parser.advance()?;

    let (token, datatype) = parse_datatype_group(parser, token, None, None, None);

    match datatype {
        Some(Datatype::Variant(Variant::String(string))) => {
            let current_tree_node = &mut parser.tree_nodes[parser.current_tree_node];
            current_tree_node.derives.insert(string);
        },

        Some(Datatype::TupleData(tuple_data)) => {
            let current_tree_node = &mut parser.tree_nodes[parser.current_tree_node];
            let derives = &mut current_tree_node.derives;

            for datatype in tuple_data {
                if let Datatype::Variant(Variant::String(string)) = datatype {
                    derives.insert(string);
                }
            }
        },

        _ => ()
    }

    let token = guarded_unwrap!(token, return None);
    parse_delimiters(parser, token)
}

pub fn parse_rsml<'a>(lexer: &'a mut logos::Lexer<'a, Token<'a>>) -> Vec<TreeNode> {
    let mut parser = Parser::new(lexer);

    let root_tree_node = TreeNode::new(0, None);
    parser.tree_nodes.push(root_tree_node);

    let mut token = guarded_unwrap!(parser.advance(), return parser.tree_nodes);

    loop {
        parser.did_advance = false;

        token = guarded_unwrap!(parse_property(&mut parser, token), break);
        token = guarded_unwrap!(parse_attribute(&mut parser, token), break);
        token = guarded_unwrap!(parse_scope_selector_start(&mut parser, token), break);
        token = guarded_unwrap!(parse_scope_open(&mut parser, token, None), break);
        token = guarded_unwrap!(parse_scope_close(&mut parser, token), break);
        token = guarded_unwrap!(parse_priority_declaration(&mut parser, token), break);
        token = guarded_unwrap!(parse_name_declaration(&mut parser, token), break);
        token = guarded_unwrap!(parse_derive_declaration(&mut parser, token), break);

        // Ensures the parser is advanced at least one time per iteration.
        // This prevents infinite loops.
        if !parser.did_advance {
            token = guarded_unwrap!(parser.advance(), break)
        }
    }

    return parser.tree_nodes;
}