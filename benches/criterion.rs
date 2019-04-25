use criterion::{
    black_box, criterion_group, criterion_main, AxisScale::Logarithmic, BatchSize, Criterion,
    ParameterizedBenchmark, PlotConfiguration,
};
use std::time::Duration;

use vecshard::{ShardExt, VecShard};

const SIZES: [usize; 9] = [
    0x10, 0x40, 0x100, 0x400, 0x1000, 0x4000, 0x1_0000, 0x4_0000, 0x10_0000,
];

fn split(c: &mut Criterion) {
    c.bench(
        "split",
        ParameterizedBenchmark::new(
            "vec",
            |b, &&size| {
                b.iter_batched(
                    || vec![0u8; size],
                    |mut vec| vec.split_off(size / 2),
                    BatchSize::LargeInput,
                )
            },
            &SIZES,
        )
        .with_function("shard", |b, &&size| {
            b.iter_batched(
                || vec![0u8; size],
                |vec| vec.split_inplace_at(size / 2),
                BatchSize::LargeInput,
            )
        })
        .sample_size(1000)
        .plot_config(PlotConfiguration::default().summary_scale(Logarithmic)),
    );
}

fn index(c: &mut Criterion) {
    c.bench(
        "index",
        ParameterizedBenchmark::new(
            "vec",
            |b, &&size| {
                let vec = vec![0u8; size];
                b.iter(|| vec[size / 2])
            },
            &[1, 10, 100, 1_000, 10_000, 100_000, 1_000_000],
        )
        .with_function("shard", |b, &&size| {
            let shard = VecShard::from(vec![0u8; size]);
            b.iter(|| shard[size / 2])
        })
        // this one is a bit silly, it doesn't need a lot of sampling since there's almost no noise
        .warm_up_time(Duration::from_millis(100))
        .measurement_time(Duration::from_millis(200))
        .plot_config(PlotConfiguration::default().summary_scale(Logarithmic)),
    );
}

fn merge(c: &mut Criterion) {
    c.bench(
        "merge",
        ParameterizedBenchmark::new(
            "vec_extend",
            |b, &&size| {
                b.iter_batched(
                    || (vec![0u8; size / 2], vec![0u8; size / 2]),
                    |(mut vec1, vec2)| vec1.extend(vec2),
                    BatchSize::LargeInput,
                )
            },
            &SIZES,
        )
        .with_function("shard_inplace", |b, &&size| {
            b.iter_batched(
                || vec![0u8; size].split_inplace_at(size / 2),
                |(shard1, shard2)| VecShard::merge(shard1, shard2),
                BatchSize::LargeInput,
            )
        })
        .with_function("shard_shuffe", |b, &&size| {
            b.iter_batched(
                || vec![0u8; size].split_inplace_at(size / 2),
                |(shard1, shard2)| VecShard::merge(shard2, shard1),
                BatchSize::LargeInput,
            )
        })
        .warm_up_time(Duration::from_secs(1))
        .sample_size(1000)
        .plot_config(PlotConfiguration::default().summary_scale(Logarithmic)),
    );
}

fn iterate(c: &mut Criterion) {
    c.bench(
        "iterate",
        ParameterizedBenchmark::new(
            "vec_into",
            |b, &&size| {
                b.iter_batched(
                    || vec![0u8; size],
                    |vec| {
                        for i in vec {
                            black_box(i);
                        }
                    },
                    BatchSize::LargeInput,
                )
            },
            &SIZES,
        )
        .with_function("vec_drain", |b, &&size| {
            b.iter_batched(
                || vec![0u8; size],
                |mut vec| {
                    for i in vec.drain(..) {
                        black_box(i);
                    }
                },
                BatchSize::LargeInput,
            )
        })
        .with_function("shard_into", |b, &&size| {
            b.iter_batched(
                || VecShard::from(vec![0u8; size]),
                |shard| {
                    for i in shard {
                        black_box(i);
                    }
                },
                BatchSize::LargeInput,
            )
        })
        .with_function("vec_ref", |b, &&size| {
            let vec = vec![0u8; size];
            b.iter(|| {
                for i in &vec {
                    black_box(i);
                }
            })
        })
        .with_function("shard_ref", |b, &&size| {
            let shard = VecShard::from(vec![0u8; size]);
            b.iter(|| {
                for i in &*shard {
                    black_box(i);
                }
            })
        })
        .warm_up_time(Duration::from_secs(1))
        .sample_size(1000)
        .plot_config(PlotConfiguration::default().summary_scale(Logarithmic)),
    );
}

criterion_group!(benches, split, index, merge, iterate);
criterion_main!(benches);
