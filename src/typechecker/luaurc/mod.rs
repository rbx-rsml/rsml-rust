use serde::de::{Deserialize};
use tokio::fs;
use std::{collections::BTreeMap, ops::{Deref, DerefMut}, path::{Path, PathBuf}};

use crate::typechecker::multibimap::MultiBiMap;

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

impl<'de> Deserialize<'de> for Aliases {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{MapAccess, Visitor};
        use std::fmt;

        struct AliasesVisitor;

        impl<'de> Visitor<'de> for AliasesVisitor {
            type Value = Aliases;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a map with an 'aliases' key")
            }

            fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut aliases = None;
                while let Some(key) = access.next_key::<String>()? {
                    if key == "aliases" {
                        aliases = Some(access.next_value()?);
                    } else {
                        let _: serde::de::IgnoredAny = access.next_value()?;
                    }
                }
                let aliases = aliases.ok_or_else(|| serde::de::Error::missing_field("aliases"))?;
                Ok(Aliases(aliases))
            }
        }

        deserializer.deserialize_map(AliasesVisitor)
    }
}

impl Aliases {
    pub fn new<S: AsRef<str>>(contents: S) -> Self {
        serde_json::from_str::<Aliases>(contents.as_ref())
            .unwrap_or_else(|_| Aliases::default())
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
    pub dependants: Dependants
}

impl Luaurc {
    pub fn new<S: AsRef<str>>(contents: S) -> Self {
        Self {
            aliases: Aliases::new(contents),
            dependants: Dependants::new()
        }
    }

    pub async fn from_path(path: &Path) -> Self {
        if let Ok(contents) = fs::read_to_string(path).await {
            Luaurc::new(&contents)
        } else {
            Luaurc { aliases: Aliases::default(), dependants: Dependants::new() }
        }
    }
}