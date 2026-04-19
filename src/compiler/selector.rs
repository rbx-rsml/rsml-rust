use crate::lexer::Token;
use crate::parser::types::SelectorNode;

pub fn build_selector_string(selectors: &[&SelectorNode]) -> String {
    let mut result = String::new();
    let mut last_token_kind: Option<SelectorTokenKind> = None;

    for selector_node in selectors {
        let SelectorNode::Token(node) = *selector_node else {
            continue;
        };
        let (kind, text) = classify_and_text(&node.token.1);

        if should_add_space(last_token_kind, kind) {
            result.push(' ');
        }

        result.push_str(&text);
        last_token_kind = Some(kind);
    }

    result
}

#[derive(Clone, Copy)]
enum SelectorTokenKind {
    Text,
    ScopeOperator,
    Comma,
    StateOrEnum,
}

fn classify_and_text<'a>(token: &'a Token<'a>) -> (SelectorTokenKind, String) {
    match token {
        Token::Identifier(s) => (SelectorTokenKind::Text, s.to_string()),
        Token::QuerySelector(s) => (SelectorTokenKind::Text, format!("@{}", s)),
        Token::ChildrenSelector => (SelectorTokenKind::ScopeOperator, ">".to_string()),
        Token::DescendantsSelector => (SelectorTokenKind::ScopeOperator, ">>".to_string()),
        Token::Comma => (SelectorTokenKind::Comma, ",".to_string()),
        Token::NameSelector(s) => (SelectorTokenKind::Text, format!("#{}", s)),
        Token::PseudoSelector(s) => (SelectorTokenKind::Text, format!("::{}", s)),
        Token::TagSelectorOrEnumPart(Some(s)) => (SelectorTokenKind::Text, format!(".{}", s)),
        Token::TagSelectorOrEnumPart(None) => (SelectorTokenKind::Text, ".".to_string()),
        Token::StateSelectorOrEnumPart(Some(s)) => {
            (SelectorTokenKind::StateOrEnum, format!(":{}", s))
        }
        Token::StateSelectorOrEnumPart(None) => (SelectorTokenKind::StateOrEnum, ":".to_string()),
        _ => (SelectorTokenKind::Text, String::new()),
    }
}

fn should_add_space(last: Option<SelectorTokenKind>, current: SelectorTokenKind) -> bool {
    let Some(last) = last else {
        return false;
    };

    if matches!(
        current,
        SelectorTokenKind::Comma | SelectorTokenKind::StateOrEnum
    ) {
        return false;
    }

    matches!(
        last,
        SelectorTokenKind::ScopeOperator | SelectorTokenKind::Text | SelectorTokenKind::Comma
    )
}
