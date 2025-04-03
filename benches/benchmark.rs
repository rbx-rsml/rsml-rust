use criterion::{criterion_group, criterion_main, Criterion};

use rbx_rsml_main::{lex_rsml as lex_rsml_main, parse_rsml as parse_rsml_main};
use rbx_rsml::{lex_rsml, parse_rsml};

static CONTENT: &'static str = include_str!("../styles.rsml");

fn rsml_dev() {
    let mut lexer = lex_rsml(CONTENT);
    let _parsed = parse_rsml(&mut lexer);
}

fn rsml_main() {
    let mut lexer = lex_rsml_main(CONTENT);
    let _parsed = parse_rsml_main(&mut lexer);
}

fn benchmark(c: &mut Criterion) {
    c.bench_function("dev", |b| b.iter(|| rsml_dev()));

    c.bench_function("main", |b| b.iter(|| rsml_main()));
}

criterion_group!(benches, benchmark);
criterion_main!(benches);