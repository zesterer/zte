use chumsky::Parser as _;
use criterion::{Criterion, criterion_group, criterion_main};
use std::{hint::black_box, path::Path};
use zte::{highlight::Regex, lang::LangPack};

fn highlight(c: &mut Criterion) {
    let s = include_str!("../src/state.rs");
    let lang = LangPack::from_file_name(Path::new("state.rs"));

    c.bench_function("highlight rust", |b| {
        b.iter(|| {
            black_box(
                lang.highlighter
                    .highlight_str(black_box(&s[..]), black_box(0..s.len())),
            )
        })
    });
}

fn regex(c: &mut Criterion) {
    let r = Regex::parser().parse(r#"[a-zA-z]*"#).unwrap().optimise();
    let r1 = Regex::parser()
        .parse(r#"[a-zA-z]*"#)
        .unwrap()
        .optimise()
        .compile();
    let r2 = Regex::parser()
        .parse(r#"[a-zA-z]*"#)
        .unwrap()
        .optimise()
        .compile2();
    let s = "abcdefghijklmnopqrstuvwxyz".repeat(10_000);

    c.bench_function("regex ast", |b| {
        b.iter(|| black_box(r.matches(black_box(&s[..]), black_box(0))))
    });
    c.bench_function("regex compile", |b| {
        b.iter(|| black_box(r1.matches(black_box(&s[..]), black_box(0))))
    });
    c.bench_function("regex compile2", |b| {
        b.iter(|| black_box(r2.matches(black_box(&s[..]), black_box(0))))
    });
}

criterion_group!(benches, highlight, regex);
criterion_main!(benches);
