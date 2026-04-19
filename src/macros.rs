#[macro_export]
macro_rules! collection {
    ($($k:expr => $v:expr),* $(,)?) => {{
        use std::iter::{Iterator, IntoIterator};
        Iterator::collect(IntoIterator::into_iter([$(($k, $v),)*]))
    }};

    ($($v:expr),* $(,)?) => {{
        use std::iter::{Iterator, IntoIterator};
        Iterator::collect(IntoIterator::into_iter([$($v,)*]))
    }};
}

#[macro_export]
macro_rules! lazy_collection {
    ($($k:expr => $v:expr),* $(,)?) => {{
        use std::iter::{Iterator, IntoIterator};
        LazyLock::new(|| Iterator::collect(IntoIterator::into_iter([$(($k, $v),)*])))
    }};

    ($($v:expr),* $(,)?) => {{
        use std::iter::{Iterator, IntoIterator};
        LazyLock::new(|| Iterator::collect(IntoIterator::into_iter([$($v,)*])))
    }};
}
