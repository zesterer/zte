use chumsky::Parser as _;
use criterion::{Criterion, criterion_group, criterion_main};
use std::{hint::black_box, path::Path};
use zte::{lang::LangPack, regex::Regex};

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
    let regex = Regex::parser()
        .parse(r#"([a-z]a)*"#)
        .unwrap()
        .optimise()
        .optimise();
    let s = format!("{}", "aabacadaeafagahaiajakalama".repeat(10_000));

    c.bench_function("regex closures", |b| {
        let r = regex.clone().compile();
        b.iter(|| {
            assert_eq!(
                black_box(r.matches(black_box(&s[..]), black_box(0))),
                Some(s.len())
            )
        })
    });
    c.bench_function("regex tables", |b| {
        let r = regex.clone().compile2();
        b.iter(|| {
            assert_eq!(
                black_box(r.matches(black_box(&s[..]), black_box(0))),
                Some(s.len())
            )
        })
    });
}

criterion_group!(benches, highlight, regex);
criterion_main!(benches);
