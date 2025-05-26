use crate::{macros::{MacroGroup, MacroTokenIterator}, string_clip::StringClip};
use crate::lexer::Token;
use guarded::guarded_unwrap;
use indexmap::IndexSet;
use num_traits::Num;
use palette::Srgb;
use tree_node_group::{AnyTreeNode, AnyTreeNodeMut, TreeNodeType};
use std::{fmt::Debug, ops::Deref, str::FromStr, sync::LazyLock};
use phf_macros::phf_map;
use rbx_types::{Color3uint8, Content, EnumItem, UDim, Variant};
use regex::Regex;

mod tree_node_group;
pub use tree_node_group::{TreeNodeGroup, TreeNode};

mod tuple;
use tuple::Tuple;

mod selector;
use selector::Selector;

mod datatype_group;
use datatype_group::DatatypeGroup;
pub use datatype_group::Datatype;

mod variants;
pub use variants::EnumItemFromNameAndValueName;

mod operator;
use operator::Operator;

mod colors {
    include!(concat!(env!("OUT_DIR"), "/colors.rs"));
}
use colors::{TAILWIND_COLORS, BRICK_COLORS, CSS_COLORS, SKIN_COLORS};

const MULTI_LINE_STRING_STRIP_LEFT_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[ \t\f]*\n+").unwrap());

type TokenWithResult<'a, R> = (Option<Token>, R);

fn parse_number_string<D>(num_str: &str) -> Result<D, <D as FromStr>::Err>
where
    D: Num + FromStr,
    <D as FromStr>::Err: Debug 
{   
    let mut num_str = num_str.to_string();
    num_str.retain(|c| !r#"_"#.contains(c));
    num_str.parse::<D>()
}

struct Parser<'a> {
    lexer: &'a mut logos::Lexer<'a, Token>,

    /// The names of the macros that the parser is currently inside of.
    /// Used to prevent infinite recursion.
    current_macros: IndexSet<(String, usize)>,

    macros: &'a MacroGroup,
    injected_macro_tokens: Vec<MacroTokenIterator<'a>>,

    tree_nodes: TreeNodeGroup,
    current_tree_node_idx: TreeNodeType,

    tuples: Vec<Tuple>,

    did_advance: bool,
}

impl<'a> Parser<'a> {
    fn new(lexer: &'a mut logos::Lexer<'a, Token>, macros: &'a MacroGroup) -> Self {
        Self {
            lexer,

            current_macros: IndexSet::new(),
            macros,

            injected_macro_tokens: vec![],

            tree_nodes: TreeNodeGroup::new(),
            current_tree_node_idx: TreeNodeType::Root,

            tuples: vec![],

            did_advance: false,
        }
    }

    fn inject_tokens(&mut self, injected_macro_tokens: MacroTokenIterator<'a>) {
        self.injected_macro_tokens.push(injected_macro_tokens);
    }

    fn core_next(&mut self) -> Option<Result<Token, ()>> {
        if let Some(last_injected_macro_tokens) = self.injected_macro_tokens.last_mut() {
            let next_injected = last_injected_macro_tokens.next();

            if next_injected.is_some() {
                next_injected

            } else {
                self.injected_macro_tokens.pop();
                self.current_macros.pop();
                self.core_next()
            }

        } else {
            self.lexer.next()
        }
    }

    fn slice(&self) -> &'a str {
        if let Some(injected_macro_tokens) = self.injected_macro_tokens.last() {
            injected_macro_tokens.slice()
        } else {
            self.lexer.slice()
        }
    }

    // The `advance` method performs work which would be redundant for:
    // `parse_comment_multi`, `parse_comment_single`, `parse_string_multi_end`.
    // So this core method serves to strip all of it away.
    fn core_advance(&mut self) -> Option<Token> {
        self.did_advance = true;

        loop {
            match self.core_next() {
                Some(Ok(token)) => break Some(token),
                None => return None,
                _ => ()
            }
        }
    }

    fn advance(self: &mut Parser<'a>) -> Option<Token> {
        let token = guarded_unwrap!(self.core_advance(), return None);

        let token = parse_comment_multi(self, token).unwrap_or(token);

        let token = parse_comment_single(self, token).unwrap_or(token);

        parse_macro_call(self, token)
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

fn parse_macro_call<'a>(parser: &mut Parser<'a>, token: Token) -> Option<Token> {
    if !matches!(token, Token::MacroCall) { return Some(token) }

    let macro_name_with_suffix = parser.slice();
    let macro_name: String = macro_name_with_suffix.chars().take(macro_name_with_suffix.chars().count() - 1).collect();

    let next_token = parser.advance();
    if !matches!(next_token, Some(Token::ParensOpen)) { return next_token }

    let mut scope_nestedness = 0usize;
    let mut parens_nestedness = 0usize;
    let mut parens_nestedness_is_zero = true;

    let mut args_tokens: Vec<Vec<(Token, &'a str)>> = vec![];
    let mut current_args_tokens: Vec<(Token, &'a str)> = vec![];
    
    let mut next_token = parser.advance()?;
    loop {
        match next_token {
            Token::ScopeOpen => scope_nestedness += 1,
            Token::ScopeClose => scope_nestedness -= 1,

            Token::ParensOpen => {
                parens_nestedness += 1;
                parens_nestedness_is_zero = false;
            },
            Token::ParensClose => {
                if parens_nestedness_is_zero {
                    if current_args_tokens.len() != 0 {
                        args_tokens.push(current_args_tokens);
                    }
                    break

                } else {
                    parens_nestedness -= 1;
                    parens_nestedness_is_zero = parens_nestedness == 0;
                }
            }

            Token::Comma => {
                if scope_nestedness == 0 && parens_nestedness_is_zero && current_args_tokens.len() != 0 {
                    args_tokens.push(current_args_tokens);
                    current_args_tokens = vec![];

                    next_token = parser.advance()?;
                    continue;
                }
            }

            _ => ()
        }
        
        current_args_tokens.push((next_token, parser.slice()));
        next_token = parser.advance()?;
    }

    let args_len = args_tokens.len();

    let macro_data = guarded_unwrap!(parser.macros.get(&macro_name, args_len), return parser.advance());

    let current_macro = (macro_name, args_len);

    // Prevents infinite recursion.
    if parser.current_macros.contains(&current_macro) {
        return parser.advance()
    }

    parser.inject_tokens(macro_data.iter(Some(args_tokens)));

    parser.current_macros.insert(current_macro);

    parser.advance()
}

fn parse_comment_multi_end<'a>(
    parser: &mut Parser<'a>, start_equals_amount: usize
) -> Option<Token> {
    // We keep advancing tokens until we find a closing multiline string
    // token with the same amount of equals signs as the start token.
    loop {
        let token = parser.core_advance()?;

        if let Token::StringMultiEnd = token {
            let end_token_value = parser.slice();
            let end_equals_amount = end_token_value.clip(1, 1).len();

            if start_equals_amount == end_equals_amount {
                return parser.core_advance()
            }
        }
    }
}

fn parse_comment_multi<'a>(parser: &mut Parser<'a>, token: Token) -> Option<Token> {
    if let Token::CommentMultiStart = token {
        let token_value = parser.slice();
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
 
    let new_tree_node_idx = parser.tree_nodes.nodes_len();
    let new_tree_node_idx_as_parent = TreeNodeType::Node(new_tree_node_idx);

    let previous_tree_node = parser.tree_nodes.get_node_mut(parser.current_tree_node_idx);
    match previous_tree_node {
        AnyTreeNodeMut::Root(node) => node.unwrap().child_rules.push(new_tree_node_idx),
        AnyTreeNodeMut::Node(node) => node.unwrap().child_rules.push(new_tree_node_idx)
    }

    let current_tree_node = TreeNode::new(parser.current_tree_node_idx, selector);
    parser.tree_nodes.add_node(current_tree_node);

    parser.current_tree_node_idx = new_tree_node_idx_as_parent;

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

    let selector = Selector::new(parser.slice(), token);

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
            
            selector.append(parser.slice(), token);

            token = parser.advance()?;

        } else { break }
    };

    return parse_scope_open(parser, token, Some(selector.content))
}


fn parse_scope_close<'a>(parser: &mut Parser<'a>, token: Token) -> Option<Token> {
    if !matches!(token, Token::ScopeClose) { return Some(token) }

    let current_tree_node = parser.tree_nodes.get(parser.current_tree_node_idx);
    match current_tree_node {
        AnyTreeNode::Node(node) => parser.current_tree_node_idx = node.unwrap().parent,
        _ => ()
    }

    return parser.advance()
}

fn parse_string_multi_end<'a>(
    parser: &mut Parser<'a>, start_equals_amount: usize
) -> TokenWithResult<'a, String> {
    let mut string_data = String::new();

    // We keep advancing tokens until we find a closing multiline string
    // token with the same amount of equals signs as the start token.
    loop {
        match parser.lexer.next() {
            Some(Ok(token)) => {
                if let Token::StringMultiEnd = token {
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

fn parse_string_multi_datatype<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    if let Token::StringMultiStart = token {
        let token_value = parser.slice();
        let start_equals_amount = token_value.clip(1, 1).len();

        let (token, string_data) = parse_string_multi_end(parser, start_equals_amount);

        // Luau strips multiline strings up until the first occurance of a newline character.
        // So we will mimic this behaviour.
        let string_data = MULTI_LINE_STRING_STRIP_LEFT_REGEX.replace(&string_data, "").to_string();
        return (token, Some(Datatype::Variant(Variant::String(string_data))))
    };

    (Some(token), None)
}

fn parse_content_datatype<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    if let Token::RobloxContent = token {
        let content = parser.slice();
        return (parser.advance(), Some(Datatype::Variant(Variant::Content(Content::from(content)))))
    }

    (Some(token), None)
}

fn parse_string_single_datatype<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    match token {
        Token::StringSingle => {
            let str = parser.slice();
            let datatype = Some(Datatype::Variant(Variant::String(str.clip(1, 1).to_string())));
            return (parser.advance(), datatype)
        },

        Token::RobloxAsset => {
            let str = parser.slice();
            let datatype = Some(Datatype::Variant(Variant::String(str.to_string())));
            return (parser.advance(), datatype)
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

    let datatype = Some(Datatype::Variant(Variant::UDim(UDim::new(
        0.0, 
        match parse_number_string::<i32>(num_str) {
            Ok(int32) => int32,
            Err(_) => parse_number_string::<f32>(num_str).unwrap() as i32
        }
    ))));

    return (parser.advance(), datatype)
}

fn parse_number_scale<'a>(parser: &mut Parser<'a>, token: Token, num_str: &str) -> TokenWithResult<'a, Option<Datatype>> {
    if !matches!(token, Token::ScaleOrOpMod) { return (Some(token), None) }

    let datatype = Some(Datatype::Variant(Variant::UDim(UDim::new(parse_number_string::<f32>(num_str).unwrap() / 100.0, 0))));

    return (parser.advance(), datatype)
}

fn parse_number_datatype<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    if let Token::Number = token {
        let token_value = parser.slice();
        let token = parser.advance();

        if let Some(token) = token {
            let parsed = parse_number_offset(parser, token, token_value);
            if parsed.1.is_some() { return parsed }

            let parsed = parse_number_scale(parser, token, token_value);
            if parsed.1.is_some() { return parsed }
        }

        let datatype = Some(Datatype::Variant(Variant::Float32(parse_number_string::<f32>(token_value).unwrap())));

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
    let mut first_stop_idx = 0;

    let mut enum_name = None;
    for idx in 0..=token_history_len {
        let token = &token_history[idx];
        
        if let Token::Text = token {
            let text = parser.slice();
            enum_name = Some(text);
            
            first_stop_idx = idx;
            break
        }
    };
    let enum_name = guarded_unwrap!(enum_name, return None);

    let mut enum_value_name = None;
    for idx in (first_stop_idx + 1)..=token_history_len {
        let token = &token_history[idx];

        if let Token::Text = token {
            let text = parser.slice();
            enum_value_name = Some(text);
            break
        }
    };
    let enum_value_name = guarded_unwrap!(enum_value_name, return None);

    let enum_item = guarded_unwrap!(EnumItem::from_name_and_value_name(enum_name, enum_value_name), return Some(Datatype::None));
    return Some(Datatype::Variant(Variant::EnumItem(enum_item)))
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

const SHORTHAND_REBINDS: phf::Map<&'static str, &'static str> = phf_map! {
    "FlexMode" => "UIFlexMode",
    "HorizontalFlex" => "UIFlexAlignment",
    "VerticalFlex" => "UIFlexAlignment"
};

fn parse_enum_shorthand<'a>(parser: &mut Parser<'a>, token: Token, key: Option<&str>) -> TokenWithResult<'a, Option<Datatype>> {
    if !matches!(token, Token::StateOrEnumIdentifier) { return (Some(token), None) }

    let token = guarded_unwrap!(parser.advance(), return (None, None));

    let enum_value_name = if let Token::Text = token { parser.slice() } else { return (Some(token), None) };

    if let Some(key) = key {
        let enum_name = SHORTHAND_REBINDS.get(key).unwrap_or(&key);

        let enum_item = guarded_unwrap!(
            EnumItem::from_name_and_value_name(enum_name, enum_value_name), return (parser.advance(), Some(Datatype::None))
        );
        return (parser.advance(), Some(Datatype::Variant(Variant::EnumItem(enum_item))))

    } else {
        let datatype = Datatype::IncompleteEnumShorthand(enum_value_name.into());
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
fn parse_preset_tailwind_color<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    if let Token::ColorTailwind = token {
        let color_name = parser.slice();
        let datatype = TAILWIND_COLORS.get(&color_name.to_lowercase())
            .and_then(|color| Some(Datatype::Oklab(**color.deref())));

        return (parser.advance(), datatype)
    }

    (Some(token), None)
}

#[allow(warnings)]
fn parse_preset_skin_color<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    if let Token::ColorSkin = token {
        let color_name = parser.slice();
        let datatype = SKIN_COLORS.get(&color_name.to_lowercase())
            .and_then(|color| Some(Datatype::Oklab(**color.deref())));

        return (parser.advance(), datatype)
    }

    (Some(token), None)
}

#[allow(warnings)]
fn parse_preset_css_color<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    if let Token::ColorCss = token {
        let color_name = parser.slice();
        let datatype = CSS_COLORS.get(&color_name.to_lowercase())
            .and_then(|color| Some(Datatype::Oklab(**color.deref())));

        return (parser.advance(), datatype)
    }

    (Some(token), None)
}

#[allow(warnings)]
fn parse_preset_brick_color<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    if let Token::ColorBrick = token {
        let color_name = parser.slice();
        let datatype = BRICK_COLORS.get(&color_name.to_lowercase())
            .and_then(|color| Some(Datatype::Oklab(**color.deref())));

        return (parser.advance(), datatype)
    }

    (Some(token), None)
}

fn parse_preset_color_datatype<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    let parsed = parse_preset_tailwind_color(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_preset_skin_color(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_preset_css_color(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_preset_brick_color(parser, token);
    if parsed.1.is_some() { return parsed }

    return (Some(token), None)
}

fn normalize_hex(hex: &str) -> String {
    let hex = hex.trim_start_matches('#');

    match hex.len() {
        3 | 6 => hex.into(),
        1..=5 => format!("{:0<6}", hex),
        _ => hex.into(),
    }
}

fn parse_hex_color_datatype<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    if let Token::ColorHex = token {
        let hex = parser.slice();

        let color: Srgb<u8> = normalize_hex(hex).parse().unwrap();
        let datatype = Datatype::Variant(Variant::Color3uint8(Color3uint8::new(
            color.red,
            color.green,
            color.blue,
        )));

        return (parser.advance(), Some(datatype))
    }

    return (None, None)
}

fn parse_color_datatype<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    let parsed = parse_preset_color_datatype(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_hex_color_datatype(parser, token);
    if parsed.1.is_some() { return parsed }

    return (Some(token), None)
}

fn parse_attribute_reference_datatype<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    if !matches!(token, Token::AttributeIdentifier) { return (Some(token), None) }

    let token = guarded_unwrap!(parser.advance(), return (None, None));

    if let Token::Text = token {
        let attribute_name = parser.slice();
        return (parser.advance(), Some(Datatype::Variant(Variant::String(format!("${}", attribute_name)))))
    }

    (Some(token), None)
}

fn resolve_static_attribute_reference<'a>(parser: &mut Parser<'a>, static_name: &str, tree_node_idx: TreeNodeType) -> Datatype {
    let tree_node = parser.tree_nodes.get(tree_node_idx);

    match tree_node {
        AnyTreeNode::Root(node) => {
            let resolved = node.unwrap().static_attributes.get(static_name);

            match resolved {
                Some(datatype) => datatype.clone(),
        
                None => Datatype::None
            }
        },

        AnyTreeNode::Node(node) => {
            let node = node.unwrap();
            let resolved = node.static_attributes.get(static_name);

            match resolved {
                Some(datatype) => datatype.clone(),
        
                None => resolve_static_attribute_reference(parser, static_name, node.parent)
            }
        }
    }
}

fn parse_static_attribute_reference_datatype<'a>(parser: &mut Parser<'a>, token: Token) -> TokenWithResult<'a, Option<Datatype>> {
    if !matches!(token, Token::StaticAttributeIdentifier) { return (Some(token), None) }

    let token = guarded_unwrap!(parser.advance(), return (None, None));

    if let Token::Text = token {
        let static_name = parser.slice();

        let resolved_datatype = resolve_static_attribute_reference(parser, static_name, parser.current_tree_node_idx);

        return (parser.advance(), Some(resolved_datatype))
    }

    (Some(token), None)
}

fn parse_datatype<'a>(
    parser: &mut Parser<'a>, mut token: Token, key: Option<&str>
) -> TokenWithResult<'a, Option<Datatype>> {
    if let (Some(tuple_token), current_tuple_idx) = parse_tuple_name(parser, token, None, None) {
        // Checking for a tuple can lead to a new token,
        // even if cases where a tuple isn't found.
        token = tuple_token;

        if let Some(current_tuple_idx) = current_tuple_idx {
            let current_tuple = parser.get_tuple(current_tuple_idx).unwrap();

            return (Some(token), Some(current_tuple.coerce_to_datatype()))
        }
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

    let parsed = parse_attribute_reference_datatype(parser, token);
    if parsed.1.is_some() { return parsed }

    let parsed = parse_static_attribute_reference_datatype(parser, token);
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
    let parent_tuple_idx = current_tuple.parent;

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
        let tuple_name = parser.slice();
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
        let property_name = parser.slice();
        let selector_token = token;

        token = guarded_unwrap!(parser.advance(), return None);

        if !matches!(token, Token::Equals) {
            return parse_scope_selector(parser, token, Selector::new(property_name, selector_token))
        };

        // We only want to parse the property if the current node is not the root.
        if let TreeNodeType::Node(node_idx) = parser.current_tree_node_idx {
            token = guarded_unwrap!(parser.advance(), return None);

            let (token, datatype) = parse_datatype_group(
                parser, token, Some(property_name), None, None
            );
            let variant = datatype.and_then(|d| d.coerce_to_variant(Some(property_name)));
    
            if let Some(variant) = variant {
                let current_tree_node = parser.tree_nodes[node_idx].as_mut().unwrap();
                current_tree_node.properties.insert(property_name.to_string(), variant);
            }
    
            let token = guarded_unwrap!(token, return None);
            return parse_delimiters(parser, token);

        } else {
            return Some(token)
        }
    }

    Some(token)
}

fn parse_static_attribute<'a>(parser: &mut Parser<'a>, mut token: Token) -> Option<Token> {
    if !matches!(token, Token::StaticAttributeIdentifier) { return Some(token) };

    token = guarded_unwrap!(parser.advance(), return None);

    if let Token::Text = token {
        let static_name = parser.slice();
        let next_token = guarded_unwrap!(parser.advance(), return None);
        
        if !matches!(next_token, Token::Equals) { return Some(token) }

        token = guarded_unwrap!(parser.advance(), return None);

        let (token, datatype) = parse_datatype_group(
            parser, token, Some(static_name), None, None
        );
        let datatype = datatype.and_then(|d| d.coerce_to_static(Some(static_name)));

        if let Some(datatype) = datatype {
            let current_tree_node = parser.tree_nodes.get_node_mut(parser.current_tree_node_idx);
            match current_tree_node {
                AnyTreeNodeMut::Root(node) => {
                    node.unwrap().static_attributes.insert(static_name.to_string(), datatype)
                },
                AnyTreeNodeMut::Node(node) => {
                    node.unwrap().static_attributes.insert(static_name.to_string(), datatype)
                }
            };
        }

        let token = guarded_unwrap!(token, return None);
        return parse_delimiters(parser, token)
    }

    Some(token)
}

fn parse_attribute<'a>(parser: &mut Parser<'a>, mut token: Token) -> Option<Token> {
    if !matches!(token, Token::AttributeIdentifier) { return Some(token) };

    token = guarded_unwrap!(parser.advance(), return None);

    if let Token::Text = token {
        let attribute_name = parser.slice();
        let next_token = guarded_unwrap!(parser.advance(), return None);
        
        if !matches!(next_token, Token::Equals) { return Some(token) }

        token = guarded_unwrap!(parser.advance(), return None);

        let (token, datatype) = parse_datatype_group(
            parser, token, Some(attribute_name), None, None
        );
        let variant = datatype.and_then(|d| d.coerce_to_variant(Some(attribute_name)));

        if let Some(variant) = variant {
            let current_tree_node = parser.tree_nodes.get_node_mut(parser.current_tree_node_idx);
            match current_tree_node {
                AnyTreeNodeMut::Root(node) => {
                    node.unwrap().attributes.insert(attribute_name.to_string(), variant)
                },
                AnyTreeNodeMut::Node(node) => {
                    node.unwrap().attributes.insert(attribute_name.to_string(), variant)
                }
            };
        }

        let token = guarded_unwrap!(token, return None);
        return parse_delimiters(parser, token)
    }

    Some(token)
}

fn parse_priority_declaration<'a>(parser: &mut Parser<'a>, token: Token) -> Option<Token> {
    if !matches!(token, Token::PriorityDeclaration) { return Some(token) }

    let token = parser.advance()?;

    // We only want to parse the priority if the current node is not the root.
    if let TreeNodeType::Node(node_idx) = parser.current_tree_node_idx {
        let (token, datatype) = parse_datatype_group(parser, token, None, None, None);

        if let Some(Datatype::Variant(Variant::Float32(float32))) = datatype {
            let current_tree_node = parser.tree_nodes[node_idx].as_mut().unwrap();
            current_tree_node.priority = Some(float32 as i32);
        }
    
        let token = guarded_unwrap!(token, return None);
        parse_delimiters(parser, token)

    } else {
        Some(token)
    }
}

fn parse_name_declaration<'a>(parser: &mut Parser<'a>, token: Token) -> Option<Token> {
    if !matches!(token, Token::NameDeclaration) { return Some(token) }

    let token = parser.advance()?;

    // We only want to parse the name if the current node is not the root.
    if let TreeNodeType::Node(node_idx) = parser.current_tree_node_idx {
        let (token, datatype) = parse_datatype_group(
            parser, token, None, None, None
        );
    
        if let Some(Datatype::Variant(Variant::String(name))) = datatype {
            let current_tree_node = parser.tree_nodes[node_idx].as_mut().unwrap();
            current_tree_node.name = Some(name);
        }
    
        let token = guarded_unwrap!(token, return None);
        parse_delimiters(parser, token)

    } else {
        Some(token)
    }
}

fn parse_ignore_derive_declaration<'a>(parser: &mut Parser<'a>, token: Token) -> Option<Token> {
    if !matches!(token, Token::DeriveDeclaration) { return Some(token) }

    let token = parser.core_advance()?;
    if !matches!(token, Token::Text) { return Some(token) }

    let token = parser.core_advance()?;

    parse_ignore_tuple_open(parser, token)
}

// Util and Macro declarations are ignored in the main parser.
fn parse_ignore_scope_open<'a>(parser: &mut Parser<'a>, token: Token) -> Option<Token> {
    if !matches!(token, Token::ScopeOpen) { return Some(token) }

    let mut nestedness = 0usize;

    loop {
        match parser.core_advance()? {
            Token::ScopeOpen => nestedness += 1,
            Token::ScopeClose => match nestedness {
                // End of parsing.
                0 => return parser.advance(),
                _ => nestedness -= 1
            },
            _ => ()
        }
    }
}

// Util declarations are ignored in the main parser.
fn parse_util_declaration<'a>(parser: &mut Parser<'a>, token: Token) -> Option<Token> {
    if !matches!(token, Token::UtilDeclaration) { return Some(token) }

    let token = parser.core_advance()?;
    if !matches!(token, Token::Text) { return Some(token) }

    let token = parser.core_advance()?;
    parse_ignore_scope_open(parser, token)
}

// Macro declarations are ignored in the main parser.
fn parse_ignore_tuple_open<'a>(parser: &mut Parser<'a>, token: Token) -> Option<Token> {
    if !matches!(token, Token::ParensOpen) { return Some(token) }

    let mut nestedness = 0usize;

    loop {
        match parser.core_advance()? {
            Token::ParensOpen => nestedness += 1,
            Token::ParensClose => match nestedness {
                // End of parsing.
                0 => return parser.advance(),
                _ => nestedness -= 1
            },
            _ => ()
        }
    }
}

// Macro declarations are ignored in the main parser.
fn parse_macro_declaration<'a>(parser: &mut Parser<'a>, token: Token) -> Option<Token> {
    if !matches!(token, Token::MacroDeclaration) { return Some(token) }

    let token = parser.core_advance()?;
    if !matches!(token, Token::Text) { return Some(token) }

    let token = parser.core_advance()?;

    let token = parse_ignore_tuple_open(parser, token)?;

    parse_ignore_scope_open(parser, token)
}

fn main_loop<'a>(parser: &mut Parser<'a>) -> Option<()> {
    let mut token = guarded_unwrap!(parser.advance(), return None);

    loop {
        parser.did_advance = false;

        token = parse_attribute(parser, token)?;
        token = parse_static_attribute(parser, token)?;
        token = parse_property(parser, token)?;
        token = parse_scope_selector_start(parser, token)?;
        token = parse_scope_open(parser, token, None)?;
        token = parse_scope_close(parser, token)?;
        token = parse_priority_declaration(parser, token)?;
        token = parse_name_declaration(parser, token)?;
        token = parse_ignore_derive_declaration(parser, token)?;
        token = parse_util_declaration(parser, token)?;
        token = parse_macro_declaration(parser, token)?;

        // Ensures the parser is advanced at least one time per iteration.
        // This prevents infinite loops.
        if !parser.did_advance {
            token = guarded_unwrap!(parser.advance(), break)
        }
    }

    None
}

pub fn parse_rsml<'a>(lexer: &'a mut logos::Lexer<'a, Token>, macros: &'a MacroGroup) -> TreeNodeGroup {
    let mut parser = Parser::<'a>::new(lexer, macros);

    main_loop(&mut parser);

    return parser.tree_nodes;
}