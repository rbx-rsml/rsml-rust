use serde::de::Deserialize;
use tokio::fs;
use std::{collections::BTreeMap, ops::{Deref, DerefMut}, path::{Path, PathBuf}};

use crate::typechecker::multibimap::MultiBiMap;
use crate::types::LanguageMode;

#[derive(Debug, Default)]
pub struct Aliases(pub BTreeMap<String, PathBuf>);

impl Deref for Aliases {
    type Target = BTreeMap<String, PathBuf>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Aliases {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Aliases {
    pub fn new<S: AsRef<str>>(contents: S) -> Self {
        Luaurc::new(contents).aliases
    }

    pub async fn from_path(path: &Path) -> Self {
        if let Ok(contents) = fs::read_to_string(path).await {
            Aliases::new(&contents)
        } else {
            Aliases::default()
        }
    }

    pub fn diff<'a>(
        &'a self,
        other: &'a Aliases
    ) -> impl Iterator<Item = &'a String> {
        let mut self_iter = self.iter();
        let mut other_iter = other.iter();

        let mut other_self = self_iter.next();
        let mut other_next = other_iter.next();

        std::iter::from_fn(move || loop {
            match (other_self, other_next) {
                (
                    Some((key_self, value_self)),
                    Some((key_other, value_other))
                ) => {
                    if key_self == key_other {
                        let out =
                            if value_self == value_other { None }
                            else { Some(key_self) };

                        other_self = self_iter.next();
                        other_next = other_iter.next();

                        if out.is_some() { return out }

                    } else if key_self < key_other {
                        let out = Some(key_self);
                        other_self = self_iter.next();

                        return out;

                    } else {
                        let out = Some(key_other);
                        other_next = other_iter.next();

                        return out;
                    }
                }

                (Some((key_self, _)), None) => {
                    other_self = self_iter.next();
                    return Some(key_self);
                }

                (None, Some((key_other, _))) => {
                    other_next = other_iter.next();
                    return Some(key_other);
                }

                (None, None) => return None,
            }
        })
    }

}

#[derive(Debug, Default)]
pub struct Dependants(MultiBiMap<String, PathBuf>);

impl Deref for Dependants {
    type Target = MultiBiMap<String, PathBuf>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Dependants {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Dependants {
    pub fn new() -> Self {
        Self(MultiBiMap::new())
    }
}

#[derive(Default, Debug)]
pub struct Luaurc {
    pub aliases: Aliases,
    pub dependants: Dependants,
    pub language_mode: LanguageMode,
}

impl<'de> Deserialize<'de> for Luaurc {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{MapAccess, Visitor};
        use std::fmt;

        struct LuaurcVisitor;

        impl<'de> Visitor<'de> for LuaurcVisitor {
            type Value = Luaurc;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a .luaurc configuration object")
            }

            fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut aliases = Aliases::default();
                let mut language_mode = LanguageMode::default();

                while let Some(key) = access.next_key::<String>()? {
                    match key.as_str() {
                        "aliases" => {
                            let map: BTreeMap<String, PathBuf> = access.next_value()?;
                            aliases = Aliases(map);
                        }
                        "languageMode" => {
                            let value: serde_json::Value = access.next_value()?;
                            language_mode = match value.as_str() {
                                Some("strict") => LanguageMode::Strict,
                                _ => LanguageMode::Nonstrict,
                            };
                        }
                        _ => {
                            let _: serde::de::IgnoredAny = access.next_value()?;
                        }
                    }
                }

                Ok(Luaurc {
                    aliases,
                    dependants: Dependants::new(),
                    language_mode,
                })
            }
        }

        deserializer.deserialize_map(LuaurcVisitor)
    }
}

impl Luaurc {
    pub fn new<S: AsRef<str>>(contents: S) -> Self {
        serde_json::from_str::<Luaurc>(contents.as_ref())
            .unwrap_or_else(|_| Luaurc::default())
    }

    pub async fn from_path(path: &Path) -> Self {
        if let Ok(contents) = fs::read_to_string(path).await {
            Luaurc::new(&contents)
        } else {
            Luaurc::default()
        }
    }
}
