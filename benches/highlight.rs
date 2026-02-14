use criterion::{Criterion, criterion_group, criterion_main};
use std::{hint::black_box, path::Path};
use zte::lang::LangPack;

fn highlight(c: &mut Criterion) {
    let s = include_str!("../src/state.rs").chars().collect::<Vec<_>>();
    let lang = LangPack::from_file_name(Path::new("state.rs"));

    c.bench_function("fib 20", |b| {
        b.iter(|| {
            black_box(
                lang.highlighter
                    .highlight_str(black_box(&s[..]), black_box(0..s.len())),
            )
        })
    });
}

criterion_group!(benches, highlight);
criterion_main!(benches);
