use crate::string_clip::StringClip;
use crate::lexer::Token;
use guarded::guarded_unwrap;
use std::{ops::Deref, sync::LazyLock};
use phf_macros::phf_map;
use rbx_types::{Color3, Content, UDim, Variant};
use regex::Regex;
use hex_color::HexColor;

mod tree_node_group;
pub use tree_node_group::{TreeNodeGroup, TreeNode};

mod tuple;
use tuple::Tuple;

mod selector;
use selector::Selector;

mod datatype_group;
use datatype_group::DatatypeGroup;
pub use datatype_group::Datatype;

mod operator;
use operator::Operator;

mod colors {
    include!(concat!(env!("OUT_DIR"), "/colors.rs"));
}
use colors::{TAILWIND_COLORS, BRICK_COLORS, CSS_COLORS};

static MULTI_LINE_STRING_STRIP_LEFT_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[ \t\f]*\n+").unwrap());

type TokenWithResult<'a, R> = (Option<Token>, R);

struct Parser<'a> {
    lexer: &'a mut logos::Lexer<'a, Token>,

    tree_nodes: TreeNodeGroup,
    current_tree_node: usize,

    tuples: Vec<Tuple>,

    did_advance: bool,
}

impl<'a> Parser<'a> {
    fn new(lexer: &'a mut logos::Lexer<'a, Token>) -> Self {
        Self {
            lexer,

            tree_nodes: TreeNodeGroup::new(),
            current_tree_node: 0,

            tuples: vec![],

            did_advance: false,
        }
    }

    // The `advance` method performs work which would be redundant for:
    // `parse_comment_multi`, `parse_comment_single`, `parse_string_multi_end`.
    // So this core method serves to strip all of it away.
    fn core_advance(&mut self) -> Option<Token> {
        self.did_advance = true;

        loop {
            match self.lexer.next() {
                Some(Ok(token)) => break Some(token),
                None => return None,
                _ => ()
            }
        }
    }

    fn advance(self: &mut Parser<'a>) -> Option<Token> {
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

fn parse_comment_multi_end<'a>(
    parser: &mut Parser<'a>, start_equals_amount: usize
) -> Option<Token> {
    // We keep advancing tokens until we find a closing multiline string
    // token with the same amount of equals signs as the start token.
    loop {
        let token = parser.core_advance()?;

        if let Token::StringMultiEnd = token {
            let end_token_value = parser.lexer.slice();
            let end_equals_amount = end_token_value.clip(1, 1).len();

            if start_equals_amount == end_equals_amount {
                return parser.core_advance()
            }
        }
    }
}

fn parse_comment_multi<'a>(parser: &mut Parser<'a>, token: Token) -> Option<Token> {
    if let Token::CommentMultiStart = token {
        let token_value = parser.lexer.slice();
        let start_equals_amount = token_value.clip(3, 1).len();

        return parse_comment_multi_end(parser, start_equals_amount);
    };

    None
}

fn parse_comment_single<'a>(parser: &mut Parser<'a>, token: Token) -> Option<Token> {
    if !matches!(token, Token::CommentSingle) { return None }

    parser.core_advance()
}

fn parse_scope_open<'a>(parser: &mut Parser<'a>, token: Token, selector: Option<String>) -> Option<Token> {
    if !matches!(token, Token::ScopeOpen) { return Some(token) }

    let new_tree_node_idx = parser.tree_nodes.len();

    let previous_tree_node = parser.tree_nodes.get_mut(parser.current_tree_node).unwrap();
    previous_tree_node.rules.push(new_tree_node_idx);

    let current_tree_node = TreeNode::new(parser.current_tree_node, selector);
    parser.tree_nodes.push(current_tree_node);

    parser.current_tree_node = new_tree_node_idx;

    parser.advance()
}

fn parse_scope_delimiter<'a>(
    parser: &mut Parser<'a>,
    token: Token,
    delim_token: Option<Token>
) -> TokenWithResult<'a, Option<Token>> {
    if matches!(token, Token::Comma) {
        let next_token = guarded_unwrap!(parser.advance(), return (None, delim_token));
        return parse_scope_delimiter(parser, next_token, Some(token))

    } else {
        return (Some(token), delim_token)
    }
}

fn parse_scope_selector_start<'a>(parser: &mut Parser<'a>, token: Token) -> Option<Token> {
    // The `Token::Text` case is handled in `parse_property`.
    if !matches!(token, 
        Token::NameIdentifier | Token::PsuedoIdentifier | Token::StateOrEnumIdentifier |
        Token::TagOrEnumIdentifier | Token::ScopeToDescendants | Token::ScopeToChildren
    ) { return Some(token) }

    let selector = Selector::new(parser.lexer.slice(), token);

    let token = parser.advance()?;
    return parse_scope_selector(parser, token, selector);
}

fn parse_scope_selector<'a>(
    parser: &mut Parser<'a>, mut token: Token, mut selector: Selector
) -> Option<Token> {
    loop {
        // Advances the parser until no delimiter token is found.
        let parsed_delimiter = parse_scope_delimiter(parser, token, None);
        token = guarded_unwrap!(parsed_delimiter.0, return None);

        if matches!(token, 
            Token::NameIdentifier | Token::PsuedoIdentifier | Token::StateOrEnumIdentifier |
            Token::TagOrEnumIdentifier | Token::ScopeToDescendants | Token::ScopeToChildren |
            Token::Text
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


fn parse_scope_close<'a>(parser: &mut Parser<'a>, token: Token) -> Option<Token> {
    if !matches!(token, Token::ScopeClose) { return Some(token) }

    let current_tree_node = parser.tree_nodes.get(parser.current_tree_node).unwrap();
    parser.current_tree_node = current_tree_node.parent;

    return parser.advance()
}

fn parse_string_multi_end<'a>(
    parser: &mut Parser<'a>, start_equals_amount: usize, token_history: &mut Vec<&'a str>
) -> Option<Token> {
    // We keep advancing tokens until we find a closing multiline string
    // token with the same amount of equals signs as the start token.
    loop {
        match parser.lexer.next() {
            Some(Ok(token)) => {
                if let Token::StringMultiEnd = token {
                    let end_token_value = parser.lexer.slice();
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

            None => return None
        };
    }
}

fn parse_string_multi_datatype<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    if let Token::StringMultiStart = token {
        let token_value = parser.lexer.slice();
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

fn parse_content_datatype<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    if let Token::RobloxContent = token {
        let content = parser.lexer.slice();
        return (parser.advance(), Some(Datatype::Variant(Variant::Content(Content::from(content)))))
    }

    (Some(token), None)
}

fn parse_string_single_datatype<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    match token {
        Token::StringSingle => {
            let str = parser.lexer.slice();
            let token = parser.advance();
            let datatype = Some(Datatype::Variant(Variant::String(str.clip(1, 1).to_string())));
            return (token, datatype)
        },

        Token::RobloxAsset => {
            let str = parser.lexer.slice();
            let token = parser.advance();
            let datatype = Some(Datatype::Variant(Variant::String(str.to_string())));
            return (token, datatype)
        },

        _ => (Some(token), None)
    }
}

fn parse_string_datatype<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    let parsed = parse_string_single_datatype(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_string_multi_datatype(parser, token);
    if parsed.1.is_some() { return parsed }

    (Some(token), None)
}

fn parse_number_offset<'a>(parser: &mut Parser<'a>, token: Token, num_str: &str) -> TokenWithResult<'a, Option<Datatype>> {
    if !matches!(token, Token::Offset) { return (Some(token), None) }

    let token = parser.advance();
    let datatype = Some(Datatype::Variant(Variant::UDim(UDim::new(
        0.0, 
        match num_str.parse::<i32>() {
            Ok(int32) => int32,
            Err(_) => num_str.parse::<f32>().unwrap() as i32
        }
    ))));

    return (token, datatype)
}

fn parse_number_scale<'a>(parser: &mut Parser<'a>, token: Token, num_str: &str) -> TokenWithResult<'a, Option<Datatype>> {
    if !matches!(token, Token::ScaleOrOpMod) { return (Some(token), None) }

    let token = parser.advance();
    let datatype = Some(Datatype::Variant(Variant::UDim(UDim::new(num_str.parse::<f32>().unwrap() / 100.0, 0))));

    return (token, datatype)
}

fn parse_number_datatype<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    if let Token::Number = token {
        let token_value = parser.lexer.slice();
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

fn parse_operator_datatype<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    match Operator::from_token(token) {
        Some(operator) => (parser.advance(), Some(Datatype::Operator(operator.clone()))),
        None => (Some(token), None)
    }
}

fn parse_enum_tokens<'a>(parser: &mut Parser<'a>, token_history: &mut Vec<Token>) -> Option<Datatype> {
    let token_history_len = token_history.len();
    let mut full_enum = "Enum".to_string();
    let mut first_stop_idx = 0;

    for idx in 0..=token_history_len {
        let token = &token_history[idx];
        
        if let Token::Text = token {
            let text = parser.lexer.slice();
            full_enum += &format!(".{}", text);
            
            first_stop_idx = idx;
            break
        }
    };

    for idx in (first_stop_idx + 1)..=token_history_len {
        let token = &token_history[idx];

        if let Token::Text = token {
            let text = parser.lexer.slice();
            full_enum += &format!(".{}", text);
            break
        }
    };

    return Some(Datatype::Variant(Variant::String(full_enum)))
}

fn parse_full_enum<'a>(
    parser: &mut Parser<'a>, token: Token, token_history: &mut Vec<Token>
) -> TokenWithResult<'a, Option<Datatype>> {
    if matches!(token, 
        Token::TagOrEnumIdentifier | Token::StateOrEnumIdentifier | Token::Text
    ) {
        let token = parser.advance();

        if let Some(token) = token {
            token_history.push(token);

            let parsed = parse_full_enum(parser, token, token_history);
            if parsed.1.is_some() { return parsed }
        }
    }

    if token_history.len() == 0 { return (Some(token), None) }

    return (Some(token), parse_enum_tokens(parser, token_history))
}


fn parse_enum_keyword<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    if !matches!(token, Token::EnumKeyword) { return (Some(token), None) }

    let token = guarded_unwrap!(parser.advance(), return (None, None) );

    let mut token_history = vec![];
    parse_full_enum(parser, token, &mut token_history)
}

static SHORTHAND_HARDCODES: phf::Map<&'static str, &'static str> = phf_map! {
    "FlexMode" => "UIFlexMode",
    "HorizontalFlex" => "UIFlexAlignment",
    "VerticalFlex" => "UIFlexAlignment"
};

fn parse_enum_shorthand<'a>(parser: &mut Parser<'a>, token: Token, key: Option<&str>) -> TokenWithResult<'a, Option<Datatype>> {
    if !matches!(token, Token::StateOrEnumIdentifier) { return (Some(token), None) }

    let token = guarded_unwrap!(parser.advance(), return (None, None));

    let enum_item = if let Token::Text = token { parser.lexer.slice() } else { return (Some(token), None) };

    if let Some(key) = key {
        let enum_name = SHORTHAND_HARDCODES.get(key).unwrap_or(&key);

        // TODO: convert this to its enum member number value (instead of a string) using an api dump.
        let datatype = Datatype::Variant(Variant::String(format!("Enum.{}.{}", enum_name, enum_item)));
        return (parser.advance(), Some(datatype))

    } else {
        let datatype = Datatype::IncompleteEnumShorthand(enum_item.into());
        return (parser.advance(), Some(datatype))
    }

    
}

fn parse_enum_datatype<'a>(parser: &mut Parser<'a>, token: Token, key: Option<&str>) -> TokenWithResult<'a, Option<Datatype>> {
    let parsed = parse_enum_keyword(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_enum_shorthand(parser, token, key);
    if parsed.1.is_some() { return parsed }

    (Some(token), None)
}

fn parse_nil_datatype<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    if !matches!(token, Token::Nil) { return (Some(token), None) }

    (parser.advance(), Some(Datatype::None))
}

fn parse_boolean_datatype<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
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
fn parse_predefined_tailwind_color<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    if let Token::ColorTailwind = token {
        let color_name = parser.lexer.slice();
        let datatype = TAILWIND_COLORS.get(&color_name.to_lowercase())
            .and_then(|color| Some(Datatype::Variant(Variant::Color3(**color.deref()))));

        return (parser.advance(), datatype)
    }

    (Some(token), None)
}

#[allow(warnings)]
fn parse_predefined_css_color<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    if let Token::ColorCss = token {
        let color_name = parser.lexer.slice();
        let datatype = CSS_COLORS.get(&color_name.to_lowercase())
            .and_then(|color| Some(Datatype::Variant(Variant::Color3(**color.deref()))));

        return (parser.advance(), datatype)
    }

    (Some(token), None)
}

#[allow(warnings)]
fn parse_predefined_brick_color<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    if let Token::ColorBrick = token {
        let color_name = parser.lexer.slice();
        let datatype = BRICK_COLORS.get(&color_name.to_lowercase())
            .and_then(|color| Some(Datatype::Variant(Variant::Color3(**color.deref()))));

        return (parser.advance(), datatype)
    }

    (Some(token), None)
}

fn parse_predefined_color_datatype<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    let parsed = parse_predefined_tailwind_color(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_predefined_css_color(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_predefined_brick_color(parser, token);
    if parsed.1.is_some() { return parsed }

    return (Some(token), None)
}

fn parse_hex_color_datatype<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    if let Token::ColorHex = token {
        let hex = parser.lexer.slice();
        let color = HexColor::parse(hex).unwrap();
        let datatype = Datatype::Variant(Variant::Color3(Color3::new(
            (color.r as f32) / 255.0,
            (color.g as f32) / 255.0,
            (color.b as f32) / 255.0,
        )));

        return (parser.advance(), Some(datatype))
    }

    return (None, None)
}

fn parse_color_datatype<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    let parsed = parse_predefined_color_datatype(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_hex_color_datatype(parser, token);
    if parsed.1.is_some() { return parsed }

    return (Some(token), None)
}

fn parse_attribute_name_datatype<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    if !matches!(token, Token::AttributeIdentifier) { return (Some(token), None) }

    let token = guarded_unwrap!(parser.advance(), return (None, None));

    if let Token::Text = token {
        let text = parser.lexer.slice();
        return (parser.advance(), Some(Datatype::Variant(Variant::String(format!("${}", text)))))
    }

    (Some(token), None)
}

fn parse_datatype<'a>(
    parser: &mut Parser<'a>, token: Token, key: Option<&str>
) -> TokenWithResult<'a, Option<Datatype>> {
    if let (Some(token), Some(current_tuple_idx)) = parse_tuple_name(parser, token, None, None) {
        let current_tuple = parser.get_tuple(current_tuple_idx).unwrap();

        return (Some(token), Some(current_tuple.coerce_to_datatype()))
    };
  
    let parsed = parse_string_datatype(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_content_datatype(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_number_datatype(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_operator_datatype(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_enum_datatype(parser, token, key);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_color_datatype(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_attribute_name_datatype(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_boolean_datatype(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_nil_datatype(parser, token);
    if parsed.1.is_some() { return parsed }

    (Some(token), None)
}

fn parse_datatype_group<'a>(
    parser: &mut Parser<'a>, token: Token, key: Option<&str>,
    mut datatype_group: Option<DatatypeGroup>, mut pending_operator: Option<Operator>
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
                let mut datatypes_exists = DatatypeGroup::ensure_then_insert(
                    datatype_group, 
                    Datatype::Operator(some_pending_operator)
                );
                pending_operator = None;
                datatypes_exists.push(datatype.clone());
                datatype_group = Some(datatypes_exists);
                
            } else {
                datatype_group = Some(DatatypeGroup::ensure_then_insert(datatype_group, datatype.clone()))
            }
        }

        if let Some(some_token) = token {
            if matches!(some_token, Token::ParensClose | Token::ScopeClose | Token::Comma | Token::SemiColon) {
                return (token, datatype_group.and_then(|mut x| Some(x.coerce_to_datatype())));

            } else {
                if let Datatype::Operator(operator) = datatype {
                    // Since our datatype was an operator we need to mark it as pending,
                    // atomising with the existing pending operator if it exists.

                    if let Some(some_pending_operator) = pending_operator {
                        if some_pending_operator.can_merge_with(&operator) {
                            pending_operator = Some(some_pending_operator.merge_with(&operator))
                        } else {
                            datatype_group = Some(DatatypeGroup::ensure_then_insert(datatype_group, Datatype::Operator(some_pending_operator)));
                            pending_operator = Some(operator)
                        }
                    } else {
                        pending_operator = Some(operator)
                    }
                }

                return parse_datatype_group(parser, some_token, key, datatype_group, pending_operator);
            }
        } else {
            return (token, datatype_group.and_then(|mut x| Some(x.coerce_to_datatype())));
        }

    } else {
        return (token, datatype_group.and_then(|mut x| Some(x.coerce_to_datatype())))
    }
}

fn parse_tuple_close<'a>(
    parser: &mut Parser<'a>, token: Token,
    current_tuple_idx: usize, root_tuple_idx: usize
) -> Option<Token> {
    if !matches!(token, Token::ParensClose) { return None }

    let token = parser.advance();

    let current_tuple = parser.get_tuple(current_tuple_idx).unwrap();
    let parent_tuple_idx = current_tuple.parent_idx;

    if let Some(some_parent_tuple_idx) = parent_tuple_idx {
        let datatype = current_tuple.coerce_to_datatype();
        parser.get_tuple_mut(some_parent_tuple_idx).unwrap().push(datatype);

        let token = guarded_unwrap!(token, return None);
        
        let parsed = parse_tuple_delimiter(parser, token, some_parent_tuple_idx, root_tuple_idx);
        if parsed.is_some() { return parsed }

        let parsed = parse_tuple_close(parser, token, some_parent_tuple_idx, root_tuple_idx);
        if parsed.is_some() { return parsed }

        let parsed = parse_tuple_name(
            parser, token, parent_tuple_idx, Some(root_tuple_idx)
        );
        if parsed.1.is_some() { return parsed.0 }
    };

    token
}

fn parse_tuple_datatype<'a>(
    parser: &mut Parser<'a>, token: Token,
    current_tuple_idx: usize, root_tuple_idx: usize
) -> Option<Token> {
    let (token, datatype) = parse_datatype_group(
        parser, token, None, None, None
    );

    let datatype = guarded_unwrap!(datatype, return None);

    let current_tuple = parser.get_tuple_mut(current_tuple_idx).unwrap();
    current_tuple.push(datatype);

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
    parser: &mut Parser<'a>, token: Token,
    current_tuple_idx: usize, root_tuple_idx: usize
) -> Option<Token> {
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
    parser: &mut Parser<'a>, token: Token, tuple_name: Option<String>,
    parent_tuple_idx: Option<usize>, root_tuple_idx: Option<usize>
) -> TokenWithResult<'a, Option<usize>> {
    if !matches!(token, Token::ParensOpen) { return (None, None) };

    let current_tuple_idx = parser.push_tuple(
        Tuple::new(tuple_name, parent_tuple_idx)
    );

    let token = guarded_unwrap!(parser.advance(), return (None, Some(current_tuple_idx)));

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
    parser: &mut Parser<'a>, token: Token,
    parent_tuple_idx: Option<usize>, root_tuple_idx: Option<usize>
) -> TokenWithResult<'a, Option<usize>> {
    if let Token::Text = token {
        let tuple_name = parser.lexer.slice();
        let token = guarded_unwrap!(parser.advance(), return (None, None));

        return parse_tuple_open(parser, token, Some(tuple_name.to_string()), parent_tuple_idx, root_tuple_idx)

    } else {
        return parse_tuple_open(parser, token, None, parent_tuple_idx, root_tuple_idx)
    }
}

fn parse_delimiters<'a>(parser: &mut Parser<'a>, token: Token)  -> Option<Token> {
    if matches!(token, Token::SemiColon | Token::Comma) {
        let token = guarded_unwrap!(parser.advance(), return None);
        return parse_delimiters(parser, token);
    }

    return Some(token)
}

fn parse_property<'a>(parser: &mut Parser<'a>, mut token: Token) -> Option<Token> {
    if let Token::Text = token {
        let property_name = parser.lexer.slice();
        let selector_token = token;

        token = guarded_unwrap!(parser.advance(), return None);

        if !matches!(token, Token::Equals) {
            return parse_scope_selector(parser, token, Selector::new(property_name, selector_token))
        };

        token = guarded_unwrap!(parser.advance(), return None);


        let (token, datatype) = parse_datatype_group(
            parser, token, Some(property_name), None, None
        );
        let variant = datatype.and_then(|d| d.coerce_to_variant(Some(property_name)));

        if let Some(variant) = variant {
            let current_tree_node = parser.tree_nodes.get_mut(parser.current_tree_node).unwrap();
            current_tree_node.properties.insert(property_name.to_string(), variant);
        }

        let token = guarded_unwrap!(token, return None);
        return parse_delimiters(parser, token);
    }

    Some(token)
}

fn parse_attribute<'a>(parser: &mut Parser<'a>, mut token: Token) -> Option<Token> {
    if !matches!(token, Token::AttributeIdentifier) { return Some(token) };

    token = guarded_unwrap!(parser.advance(), return None);

    if let Token::Text = token {
        let attribute_name = parser.lexer.slice();
        let next_token = guarded_unwrap!(parser.advance(), return None);
        
        if !matches!(next_token, Token::Equals) { return Some(token) }

        token = guarded_unwrap!(parser.advance(), return None);

        let (token, datatype) = parse_datatype_group(
            parser, token, Some(attribute_name), None, None
        );
        let variant = datatype.and_then(|d| d.coerce_to_variant(Some(attribute_name)));

        if let Some(variant) = variant {
            let current_tree_node = parser.tree_nodes.get_mut(parser.current_tree_node).unwrap();
            current_tree_node.attributes.insert(attribute_name.to_string(), variant);
        }

        let token = guarded_unwrap!(token, return None);
        return parse_delimiters(parser, token)
    }

    Some(token)
}

fn parse_priority_declaration<'a>(parser: &mut Parser<'a>, token: Token) -> Option<Token> {
    if !matches!(token, Token::PriorityDeclaration) { return Some(token) }

    let token = parser.advance()?;

    let (token, datatype) = parse_datatype_group(parser, token, None, None, None);

    if let Some(Datatype::Variant(Variant::Float32(float32))) = datatype {
        let current_tree_node = parser.tree_nodes.get_mut(parser.current_tree_node).unwrap();
        current_tree_node.priority = Some(float32 as i32);
    }

    let token = guarded_unwrap!(token, return None);
    return parse_delimiters(parser, token)
}

fn parse_name_declaration<'a>(parser: &mut Parser<'a>, token: Token) -> Option<Token> {
    if !matches!(token, Token::NameDeclaration) { return Some(token) }

    let token = parser.advance()?;

    let (token, datatype) = parse_datatype_group(
        parser, token, None, None, None
    );

    if let Some(Datatype::Variant(Variant::String(name))) = datatype {
        let current_tree_node = parser.tree_nodes.get_mut(parser.current_tree_node).unwrap();
        current_tree_node.name = Some(name);
    }

    let token = guarded_unwrap!(token, return None);
    return parse_delimiters(parser, token)
}

fn parse_derive_declaration<'a>(parser: &mut Parser<'a>, token: Token) -> Option<Token> {
    if !matches!(token, Token::DeriveDeclaration) { return Some(token) }

    let token = parser.advance()?;

    let (token, datatype) = parse_datatype_group(parser, token, None, None, None);

    match datatype {
        Some(Datatype::Variant(Variant::String(string))) => {
            let current_tree_node = parser.tree_nodes.get_mut(parser.current_tree_node).unwrap();
            current_tree_node.derives.insert(string);
        },

        Some(Datatype::TupleData(tuple_data)) => {
            let current_tree_node = parser.tree_nodes.get_mut(parser.current_tree_node).unwrap();
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

fn main_loop(mut parser: &mut Parser) -> Option<()> {
    let mut token = guarded_unwrap!(parser.advance(), return None);

    loop {
        parser.did_advance = false;

        token = parse_attribute(&mut parser, token)?;
        token = parse_property(&mut parser, token)?;
        token = parse_scope_selector_start(&mut parser, token)?;
        token = parse_scope_open(&mut parser, token, None)?;
        token = parse_scope_close(&mut parser, token)?;
        token = parse_priority_declaration(&mut parser, token)?;
        token = parse_name_declaration(&mut parser, token)?;
        token = parse_derive_declaration(&mut parser, token)?;

        // Ensures the parser is advanced at least one time per iteration.
        // This prevents infinite loops.
        if !parser.did_advance {
            token = guarded_unwrap!(parser.advance(), break)
        }
    }

    None
}

pub fn parse_rsml<'a>(lexer: &'a mut logos::Lexer<'a, Token>) -> TreeNodeGroup {
    let mut parser = Parser::new(lexer);

    let root_tree_node = TreeNode::new(0, None);
    parser.tree_nodes.push(root_tree_node);

    main_loop(&mut parser);

    return parser.tree_nodes;
}