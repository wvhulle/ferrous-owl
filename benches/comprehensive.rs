use std::{hint::black_box, path::Path, process::Command};

use criterion::{Criterion, criterion_group, criterion_main};

fn bench_ferrous_owl_check(c: &mut Criterion) {
    let binary_path = "./target/release/ferrous-owl";

    assert!(
        Path::new(binary_path).exists(),
        "Binary not found at {binary_path}. Run 'cargo build --release --bin ferrous-owl' first."
    );

    let test_fixture = "./benches/dummy";

    c.bench_function("ferrous_owl_check", |b| {
        b.iter(|| {
            let output = Command::new(binary_path)
                .args(["check", test_fixture, "--all-targets", "--all-features"])
                .output()
                .expect("Failed to run ferrous-owl check");
            black_box(output.status.success());
        });
    });
}

criterion_group!(benches, bench_ferrous_owl_check);
criterion_main!(benches);
