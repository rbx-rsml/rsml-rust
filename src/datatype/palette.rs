use std::collections::BTreeMap;
use std::sync::LazyLock;

use crate::datatype::colors::{BRICK_COLORS, CSS_COLORS, SKIN_COLORS, TAILWIND_COLORS};

/// Groups color-palette keys like `"tw:red:500"` into `("red", "500")` pairs
/// so autocomplete can surface families and shades independently.
struct TwoLevelIndex {
    families: Vec<&'static str>,
    shades_by_family: BTreeMap<&'static str, Vec<&'static str>>,
}

impl TwoLevelIndex {
    fn build<I>(keys: I, prefix: &'static str) -> Self
    where
        I: Iterator<Item = &'static &'static str>,
    {
        let mut shades_by_family: BTreeMap<&'static str, Vec<&'static str>> = BTreeMap::new();

        for key in keys {
            let Some(rest) = key.strip_prefix(prefix) else {
                continue;
            };

            let mut parts = rest.splitn(2, ':');
            let Some(family) = parts.next() else {
                continue;
            };

            let entry = shades_by_family.entry(family).or_default();

            if let Some(shade) = parts.next() {
                entry.push(shade);
            }
        }

        for shades in shades_by_family.values_mut() {
            shades.sort_by_key(|shade| shade.parse::<u32>().unwrap_or(u32::MAX));
            shades.dedup();
        }

        let families = shades_by_family.keys().copied().collect();

        Self { families, shades_by_family }
    }
}

struct OneLevelIndex {
    names: Vec<&'static str>,
}

impl OneLevelIndex {
    fn build<I>(keys: I, prefix: &'static str) -> Self
    where
        I: Iterator<Item = &'static &'static str>,
    {
        let mut names: Vec<&'static str> = keys
            .filter_map(|key| key.strip_prefix(prefix))
            .collect();

        names.sort();
        names.dedup();

        Self { names }
    }
}

static TAILWIND_INDEX: LazyLock<TwoLevelIndex> =
    LazyLock::new(|| TwoLevelIndex::build(TAILWIND_COLORS.keys(), "tw:"));

static SKIN_INDEX: LazyLock<TwoLevelIndex> =
    LazyLock::new(|| TwoLevelIndex::build(SKIN_COLORS.keys(), "skin:"));

static BRICK_INDEX: LazyLock<OneLevelIndex> =
    LazyLock::new(|| OneLevelIndex::build(BRICK_COLORS.keys(), "bc:"));

static CSS_INDEX: LazyLock<OneLevelIndex> =
    LazyLock::new(|| OneLevelIndex::build(CSS_COLORS.keys(), "css:"));

pub fn tailwind_families() -> &'static [&'static str] {
    &TAILWIND_INDEX.families
}

pub fn tailwind_shades(family: &str) -> &'static [&'static str] {
    TAILWIND_INDEX
        .shades_by_family
        .get(family)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

pub fn skin_families() -> &'static [&'static str] {
    &SKIN_INDEX.families
}

pub fn skin_shades(family: &str) -> &'static [&'static str] {
    SKIN_INDEX
        .shades_by_family
        .get(family)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

pub fn brick_names() -> &'static [&'static str] {
    &BRICK_INDEX.names
}

pub fn css_names() -> &'static [&'static str] {
    &CSS_INDEX.names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tailwind_has_red_family() {
        assert!(tailwind_families().contains(&"red"));
    }

    #[test]
    fn tailwind_red_has_500_shade() {
        assert!(tailwind_shades("red").contains(&"500"));
    }

    #[test]
    fn tailwind_shades_unknown_family_empty() {
        assert!(tailwind_shades("notafamily").is_empty());
    }

    #[test]
    fn skin_has_rose_family() {
        assert!(skin_families().contains(&"rose"));
    }

    #[test]
    fn css_has_red() {
        assert!(css_names().contains(&"red"));
    }

    #[test]
    fn brick_has_white() {
        assert!(brick_names().contains(&"white"));
    }
}
