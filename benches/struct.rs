/*
    Copyright (C) 2023 Aurora McGinnis

    This Source Code Form is subject to the terms of the Mozilla Public
    License, v. 2.0. If a copy of the MPL was not distributed with this
    file, You can obtain one at http://mozilla.org/MPL/2.0/.

    struct.rs: Benchmark struct serialization performance.
*/

use alogfmt::to_string;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use serde::Serialize;

#[derive(Serialize, Debug)]
struct MyStruct {
    a: u64,
    b: String,
    c: [u8; 32],
}

fn criterion_benchmark(c: &mut Criterion) {
    let input1 = MyStruct {
        a: 482942,
        b: String::from("Did you ever hear the tragedy of Darth Plagueis the Wise? I thought not. It's not a story the Jedi would tell you. It's a Sith legend. Darth Plagueis was a Dark Lord of the Sith, so powerful and so wise he could use the Force to influence the midichlorians to create life... He had such a knowledge of the dark side that he could even keep the ones he cared about from dying. The dark side of the Force is a pathway to many abilities some consider to be unnatural. He became so powerful... the only thing he was afraid of was losing his power, which eventually, of course, he did. Unfortunately, he taught his apprentice everything he knew, then his apprentice killed him in his sleep. Ironic, he could save others from death, but not himself."),
        c: [0xFF; 32],
    };

    c.bench_with_input(
        BenchmarkId::new("serialize_struct", "large"),
        &input1,
        |b, i| b.iter(|| to_string(i)),
    );

    let input2 = MyStruct {
        a: 482942,
        b: String::from("blah"),
        c: [0xFF; 32],
    };

    c.bench_with_input(
        BenchmarkId::new("serialize_struct", "small"),
        &input2,
        |b, i| b.iter(|| to_string(i)),
    );

    let input3 = MyStruct {
        a: 482942,
        b: String::from("blah \n \t \\n"),
        c: [0xFF; 32],
    };

    c.bench_with_input(
        BenchmarkId::new("serialize_struct", "small w/ escapes"),
        &input3,
        |b, i| b.iter(|| to_string(i)),
    );
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
