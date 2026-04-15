use std::{mem::{self, Discriminant, MaybeUninit}};
use crate::lexer::{Token, TokenKind};

#[macro_export]
macro_rules! token_kind_list {
    ($str:literal, [ $( $name:ident ),* ]) => {
        &TokenKindList::new_with_stringified([$(
            (TokenKind::$name, std::mem::discriminant(&TokenKind::$name))
        ),*], Stringified::Single(String::from($str)))
    };

    ($( $name:ident ),*) => {
        &TokenKindList::new([$(
            (TokenKind::$name, std::mem::discriminant(&TokenKind::$name))
        ),*])
    };

    ([ $( $name:ident ),* ]) => {
        token_kind_list!($( $name ),*)
    };
}

#[derive(Debug)]
pub struct TokenKindList<const N: usize>{
    items: [(TokenKind, Discriminant<TokenKind>); N],
    stringified: Option<Stringified>
}

#[derive(Debug, Clone)]
pub enum Stringified {
    Single(String),
    Many(Vec<String>)
}

impl Stringified {
    fn iter(&self) -> Box<dyn Iterator<Item = &String> + '_> {
        match self {
            Stringified::Single(item) => Box::new(std::iter::once(item)),
            Stringified::Many(items) => Box::new(items.iter())
        }
    }

    fn extend(self, extend_with: Stringified) -> Stringified {
        Self::Many(match (self, extend_with) {
            (Stringified::Single(a), Stringified::Single(b)) => vec![a, b],
            (Stringified::Many(mut a), Stringified::Many(b)) => {
                a.extend(b);
                a
            },
            (Stringified::Many(mut a), Stringified::Single(b)) |
            (Stringified::Single(b), Stringified::Many(mut a)) => {
                a.push(b);
                a
            }
        })
    }

    fn push(self, to_push: String) -> Stringified {
        match self {
            Stringified::Single(item) => Stringified::Many(vec![item, to_push]),
            Stringified::Many(mut items) => Stringified::Many({
                items.push(to_push);
                items
            })
        }
    }
}

impl<'a, const N: usize> TokenKindList<N> {
    pub fn new(items: [(TokenKind, Discriminant<TokenKind>); N]) -> Self {
        Self {
            items,
            stringified: None
        }
    }

    pub fn new_with_stringified(
        items: [(TokenKind, Discriminant<TokenKind>); N],
        stringified: Stringified
    ) -> Self {
        Self {
            items,
            stringified: Some(stringified)
        }
    }

    pub fn has_token(&self, token: &Token) -> bool {
        let discriminant = &token.discriminant();

        self.items.iter().any(|x| &x.1 == discriminant)
    }

    pub fn has_discriminant(&self, discriminant: &Discriminant<TokenKind>) -> bool {
        self.items.iter().any(|x| &x.1 == discriminant)
    }

    pub fn to_string(&'a self) -> Option<String> {
        let stringified = &self.stringified;
        let items = self.items;

        let mut iter: Box<dyn Iterator<Item = &str>> =
            if let Some(stringified) = stringified { Box::new(stringified.iter().map(String::as_str)) }
            else { Box::new(items.iter().map(|x| x.0.name())) };

        let Some(mut current_item) = iter.next() else { return None };

        let mut msg = format!("{}", current_item);

        let Some(next_item) = iter.next() else { return Some(msg) };
        current_item = next_item;

        loop {
            let next_item = iter.next();
            
            if let Some(next_item) = next_item {
                let msg_item = format!(", {}", current_item);
                current_item = next_item;
                msg += &msg_item;

            } else {
                let msg_item = format!(" or {}", current_item);
                msg += &msg_item;
                break
            };
        }

        return Some(msg)
    }

    pub fn with_stringified(&self, stringified: Stringified) -> Self {
        Self {
            items: self.items,
            stringified: Some(stringified)
        }
    }

    pub fn concat<const B: usize>(&self, b: &TokenKindList<B>) -> TokenKindList<{N + B}>
    where
        [(); N + B]:
    {
        TokenKindList::<{N + B}> {
            items: array_concat(&self.items, &b.items),
            stringified: match (&self.stringified, &b.stringified) {
                (Some(a), Some(b)) => Some(a.clone().extend(b.clone())),
                (Some(a), None) => Some(a.clone()),
                (None, Some(b)) => Some(b.clone()),
                (None, None) => None
            }
        }
    }
}


fn array_concat<T, const AN: usize, const BN: usize>(a: &[T; AN], b: &[T; BN]) -> [T; AN + BN]
where T: Copy, [(); AN + BN]: {
    let mut output: [MaybeUninit<T>; AN + BN] =
        unsafe { MaybeUninit::uninit().assume_init() };

    for i in 0..AN {
        output[i] = MaybeUninit::new(a[i]);
    }

    for i in 0..BN {
        output[AN + i] = MaybeUninit::new(b[i]);
    }

    unsafe {
        mem::transmute_copy::<_, [T; AN + BN]>(&output)
    }
}