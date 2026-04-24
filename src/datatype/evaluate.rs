use palette::Srgb;
use rbx_types::{
    Color3uint8, Content, EasingDirection, EasingStyle, EnumItem, TweenInfo, UDim, Variant,
};
use rbx_types_ops::BasicOperations;

use crate::lexer::Token;
use crate::parser::types::{Construct, Delimited, Node};

use crate::datatype::colors::{BRICK_COLORS, CSS_COLORS, SKIN_COLORS, TAILWIND_COLORS};
use crate::datatype::lookup::StaticLookup;
use crate::datatype::tuple;
use crate::datatype::types::Datatype;
use crate::datatype::variants::EnumItemFromNameAndValueName;

pub fn evaluate_construct(
    construct: &Construct,
    key: Option<&str>,
    lookup: &dyn StaticLookup,
) -> Option<Datatype> {
    match construct {
        Construct::Node { node } => evaluate_token(node, key, lookup),

        Construct::MathOperation {
            left,
            operators,
            right,
        } => {
            let left_val = evaluate_construct(left, key, lookup)?;
            let right_val = right
                .as_ref()
                .and_then(|r| evaluate_construct(r, key, lookup));

            let Some(right_val) = right_val else {
                return Some(left_val);
            };

            let left_variant = left_val.coerce_to_variant(key)?;
            let right_variant = right_val.coerce_to_variant(key)?;

            let result = if let Some(first_op) = operators.first() {
                apply_operator(first_op, &left_variant, &right_variant)
            } else {
                None
            };

            result.map(Datatype::Variant)
        }

        Construct::UnaryMinus { operand, .. } => {
            let val = evaluate_construct(operand, key, lookup)?;
            let variant = val.coerce_to_variant(key)?;
            negate_variant(&variant).map(Datatype::Variant)
        }

        Construct::Table { body } => {
            let datatypes = evaluate_delimited_to_vec(body, lookup);
            coerce_tuple_data(datatypes, None)
        }

        Construct::AnnotatedTable { annotation, body } => {
            let annotation_name = match annotation.token.value() {
                Token::Identifier(name) => Some(*name),
                _ => None,
            };

            if let Some(body) = body {
                let datatypes = evaluate_delimited_to_vec(body, lookup);
                coerce_tuple_data(datatypes, annotation_name)
            } else {
                coerce_tuple_data(vec![], annotation_name)
            }
        }

        Construct::Enum { name, variant, .. } => {
            let enum_name = name.as_ref().and_then(|n| match n.token.value() {
                Token::TagSelectorOrEnumPart(Some(s)) => Some(*s),
                _ => None,
            });

            let enum_value = variant.as_ref().and_then(|v| match v.token.value() {
                Token::Identifier(s) => Some(*s),
                Token::TagSelectorOrEnumPart(Some(s)) => Some(*s),
                Token::StateSelectorOrEnumPart(Some(s)) => Some(*s),
                _ => None,
            });

            match (enum_name, enum_value) {
                (Some(name), Some(value)) => EnumItem::from_name_and_value_name(name, value)
                    .map(|item| Datatype::Variant(Variant::EnumItem(item)))
                    .or(Some(Datatype::None)),
                _ => Some(Datatype::None),
            }
        }

        Construct::Assignment { right, .. } => right
            .as_ref()
            .and_then(|r| evaluate_construct(r, key, lookup)),

        Construct::Tween { body, .. } => {
            let body = body.as_deref()?;

            let info = match body {
                Construct::Table {
                    body: Delimited { content: Some(items), .. },
                } => {
                    let args: Vec<&Construct> =
                        items.iter().filter(|item| !is_comma(item)).collect();
                    tween_info_from_args(&args, lookup)?
                }
                _ => {
                    let datatype = evaluate_construct(body, None, lookup)?;
                    tween_info_from_bare(datatype.coerce_to_variant(None)?)?
                }
            };

            Some(Datatype::Variant(Variant::TweenInfo(info)))
        }

        _ => None,
    }
}

fn evaluate_token(node: &Node, key: Option<&str>, lookup: &dyn StaticLookup) -> Option<Datatype> {
    match node.token.value() {
        Token::Number(s) => parse_number_str(s).map(|n| Datatype::Variant(Variant::Float64(n))),

        Token::NumberOffset(s) => {
            let num_str = s.strip_suffix("px").unwrap_or(s);
            let offset = parse_number_str(num_str).map(|n| n as i32).unwrap_or(0);
            Some(Datatype::Variant(Variant::UDim(UDim::new(0.0, offset))))
        }

        Token::NumberScale(s) => {
            let num_str = s.strip_suffix('%').unwrap_or(s);
            let scale = parse_number_str(num_str).unwrap_or(0.0) / 100.0;
            Some(Datatype::Variant(Variant::UDim(UDim::new(scale as f32, 0))))
        }

        Token::StringSingle(s) => Some(Datatype::Variant(Variant::String(s.to_string()))),

        Token::StringMulti(multi) => Some(Datatype::Variant(Variant::String(
            multi.content.to_string(),
        ))),

        Token::RbxAsset(slice) => Some(Datatype::Variant(Variant::String(slice.to_string()))),

        Token::RbxContent(slice) => Some(Datatype::Variant(Variant::Content(Content::from(
            slice.to_string(),
        )))),

        Token::Boolean(s) => {
            let val = *s == "true";
            Some(Datatype::Variant(Variant::Bool(val)))
        }

        Token::Nil => Some(Datatype::None),

        Token::ColorHex(slice) => {
            let hex = normalize_hex(slice);
            let color: Result<Srgb<u8>, _> = hex.parse();
            color.ok().map(|c| {
                Datatype::Variant(Variant::Color3(
                    Color3uint8::new(c.red, c.green, c.blue).into(),
                ))
            })
        }

        Token::ColorTailwind(slice) => TAILWIND_COLORS
            .get(&slice.to_lowercase())
            .map(|color| Datatype::Oklab(***color)),

        Token::ColorSkin(slice) => SKIN_COLORS
            .get(&slice.to_lowercase())
            .map(|color| Datatype::Oklab(***color)),

        Token::ColorCss(slice) => CSS_COLORS
            .get(&slice.to_lowercase())
            .map(|color| Datatype::Oklab(***color)),

        Token::ColorBrick(slice) => BRICK_COLORS
            .get(&slice.to_lowercase())
            .map(|color| Datatype::Oklab(***color)),

        Token::TokenIdentifier(attr_name) => Some(lookup.resolve_dynamic(attr_name)),

        Token::StaticTokenIdentifier(static_name) => Some(lookup.resolve_static(static_name)),

        Token::MacroArgIdentifier(Some(name)) => lookup.resolve_macro_arg(name, key),

        Token::StateSelectorOrEnumPart(Some(value)) => {
            if let Some(key) = key {
                let rebinded_key = shorthand_rebind(key);
                EnumItem::from_name_and_value_name(rebinded_key, value)
                    .map(|item| Datatype::Variant(Variant::EnumItem(item)))
                    .or(Some(Datatype::None))
            } else {
                Some(Datatype::IncompleteEnumShorthand(value.to_string()))
            }
        }

        _ => None,
    }
}

fn evaluate_delimited_to_vec(delimited: &Delimited, lookup: &dyn StaticLookup) -> Vec<Datatype> {
    let Some(content) = &delimited.content else {
        return vec![];
    };

    content
        .iter()
        .filter_map(|c| evaluate_construct(c, None, lookup))
        .collect()
}

fn coerce_tuple_data(datatypes: Vec<Datatype>, name: Option<&str>) -> Option<Datatype> {
    let mut t = tuple::Tuple::new(name.map(|s| s.to_string()));
    for d in datatypes {
        t.push(d);
    }
    let result = t.coerce_to_datatype();
    match result {
        Datatype::None => None,
        other => Some(other),
    }
}

fn negate_variant(variant: &Variant) -> Option<Variant> {
    match variant {
        Variant::Float64(n) => Some(Variant::Float64(-n)),
        Variant::UDim(udim) => Some(Variant::UDim(UDim::new(-udim.scale, -udim.offset))),
        _ => None,
    }
}

fn apply_operator(op_node: &Node, left: &Variant, right: &Variant) -> Option<Variant> {
    let narrowed_right;
    let right = match (left, right) {
        (Variant::Float64(_), _) => right,
        (_, Variant::Float64(n)) => {
            narrowed_right = Variant::Float32(*n as f32);
            &narrowed_right
        }
        _ => right,
    };

    match op_node.token.value() {
        Token::OpAdd => left.add(right),
        Token::OpSub => left.sub(right),
        Token::OpMult => left.mult(right),
        Token::OpDiv => left.div(right),
        Token::OpFloorDiv => left.floor_div(right),
        Token::OpMod => left.modulus(right),
        Token::OpPow => left.pow(right),
        _ => None,
    }
}

fn normalize_hex(hex: &str) -> String {
    let hex = hex.trim_start_matches('#');
    match hex.len() {
        3 | 6 => hex.into(),
        1..=5 => format!("{:0<6}", hex),
        _ => hex.into(),
    }
}

const SHORTHAND_REBINDS: phf::Map<&'static str, &'static str> = phf_macros::phf_map! {
    "FlexMode" => "UIFlexMode",
    "HorizontalFlex" => "UIFlexAlignment",
    "VerticalFlex" => "UIFlexAlignment",
};

pub(crate) fn shorthand_rebind<'a>(key: &'a str) -> &'a str {
    SHORTHAND_REBINDS.get(key).copied().unwrap_or(key)
}

fn parse_number_str(s: &str) -> Option<f64> {
    let cleaned: String = s.chars().filter(|c| *c != '_').collect();
    cleaned.parse::<f64>().ok()
}

fn is_comma(construct: &Construct) -> bool {
    matches!(construct, Construct::Node { node } if matches!(node.token.value(), Token::Comma))
}

fn variant_to_f32(v: &Variant) -> Option<f32> {
    match v {
        Variant::Float32(n) => Some(*n),
        Variant::Float64(n) => Some(*n as f32),
        Variant::Int32(n) => Some(*n as f32),
        Variant::Int64(n) => Some(*n as f32),
        _ => None,
    }
}

fn variant_to_i32(v: &Variant) -> Option<i32> {
    match v {
        Variant::Int32(n) => Some(*n),
        Variant::Int64(n) => Some(*n as i32),
        Variant::Float32(n) => Some(*n as i32),
        Variant::Float64(n) => Some(*n as i32),
        _ => None,
    }
}

fn datatype_to_easing_style(d: Datatype) -> Option<EasingStyle> {
    match d {
        Datatype::Variant(Variant::EnumItem(item)) => EasingStyle::from_u8(item.value as u8),
        _ => None,
    }
}

fn datatype_to_easing_direction(d: Datatype) -> Option<EasingDirection> {
    match d {
        Datatype::Variant(Variant::EnumItem(item)) => EasingDirection::from_u8(item.value as u8),
        _ => None,
    }
}

fn tween_info_from_bare(variant: Variant) -> Option<TweenInfo> {
    match variant {
        Variant::Float32(t) => Some(TweenInfo::from_time(t)),
        Variant::Float64(t) => Some(TweenInfo::from_time(t as f32)),
        Variant::Int32(t) => Some(TweenInfo::from_time(t as f32)),
        Variant::Int64(t) => Some(TweenInfo::from_time(t as f32)),
        Variant::TweenInfo(info) => Some(info),
        _ => None,
    }
}

fn tween_info_from_args(
    args: &[&Construct],
    lookup: &dyn StaticLookup,
) -> Option<TweenInfo> {
    if args.is_empty() || args.len() > 6 {
        return None;
    }

    let time = {
        let datatype = evaluate_construct(args[0], None, lookup)?;
        variant_to_f32(&datatype.coerce_to_variant(None)?)?
    };

    let easing_style = match args.get(1) {
        Some(arg) => {
            datatype_to_easing_style(evaluate_construct(arg, Some("EasingStyle"), lookup)?)?
        }
        None => EasingStyle::default(),
    };

    let easing_direction = match args.get(2) {
        Some(arg) => datatype_to_easing_direction(evaluate_construct(
            arg,
            Some("EasingDirection"),
            lookup,
        )?)?,
        None => EasingDirection::default(),
    };

    let repeat_count = match args.get(3) {
        Some(arg) => {
            let datatype = evaluate_construct(arg, None, lookup)?;
            variant_to_i32(&datatype.coerce_to_variant(None)?)?
        }
        None => 0,
    };

    let reverses = match args.get(4) {
        Some(arg) => {
            let datatype = evaluate_construct(arg, None, lookup)?;
            match datatype.coerce_to_variant(None)? {
                Variant::Bool(b) => b,
                _ => return None,
            }
        }
        None => false,
    };

    let delay_time = match args.get(5) {
        Some(arg) => {
            let datatype = evaluate_construct(arg, None, lookup)?;
            variant_to_f32(&datatype.coerce_to_variant(None)?)?
        }
        None => 0.0,
    };

    Some(TweenInfo::new(
        time,
        easing_style,
        easing_direction,
        repeat_count,
        reverses,
        delay_time,
    ))
}
