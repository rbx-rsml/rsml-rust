use std::path::{Component, Path, PathBuf};

pub trait NormalizePath {
    fn normalize(&self) -> PathBuf;
}

impl<T: AsRef<Path>> NormalizePath for T {
    fn normalize(&self) -> PathBuf {
        let mut components = self.as_ref().components().peekable();

        let mut normalized = if let Some(prefix @ Component::Prefix(..)) = components.peek().cloned() {
            components.next();
            PathBuf::from(prefix.as_os_str())
        } else {
            PathBuf::new()
        };

        for component in components {
            match component {
                Component::Prefix(..) => unreachable!(),

                Component::RootDir => {
                    normalized.push(std::path::MAIN_SEPARATOR.to_string());
                }

                Component::CurDir => {}

                Component::ParentDir => {
                    if normalized.ends_with("..") {
                        normalized.push("..");
                        continue;
                    }

                    let popped = normalized.pop();
                    if !popped && !normalized.has_root() {
                        normalized.push("..");
                    }
                }

                Component::Normal(segment) => {
                    normalized.push(segment);
                }
            }
        }

        normalized
    }
}
