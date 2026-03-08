use criterion::{criterion_group, criterion_main, Criterion};
use oximedia_simd::{
    detect_cpu_features, forward_dct, interpolate, inverse_dct, sad, BlockSize, DctSize,
    InterpolationFilter,
};
use std::hint::black_box;

fn bench_dct_forward(c: &mut Criterion) {
    let mut group = c.benchmark_group("forward_dct");

    // Detect CPU features to show which implementation is being tested
    let features = detect_cpu_features();
    eprintln!("CPU Features: {features:?}");

    // 4x4 DCT
    let input_4x4 = vec![100i16; 16];
    let mut output_4x4 = vec![0i16; 16];
    group.bench_function("4x4", |b| {
        b.iter(|| {
            forward_dct(
                black_box(&input_4x4),
                black_box(&mut output_4x4),
                black_box(DctSize::Dct4x4),
            )
        });
    });

    // 8x8 DCT
    let input_8x8 = vec![100i16; 64];
    let mut output_8x8 = vec![0i16; 64];
    group.bench_function("8x8", |b| {
        b.iter(|| {
            forward_dct(
                black_box(&input_8x8),
                black_box(&mut output_8x8),
                black_box(DctSize::Dct8x8),
            )
        });
    });

    // 16x16 DCT
    let input_16x16 = vec![100i16; 256];
    let mut output_16x16 = vec![0i16; 256];
    group.bench_function("16x16", |b| {
        b.iter(|| {
            forward_dct(
                black_box(&input_16x16),
                black_box(&mut output_16x16),
                black_box(DctSize::Dct16x16),
            )
        });
    });

    // 32x32 DCT
    let input_32x32 = vec![100i16; 1024];
    let mut output_32x32 = vec![0i16; 1024];
    group.bench_function("32x32", |b| {
        b.iter(|| {
            forward_dct(
                black_box(&input_32x32),
                black_box(&mut output_32x32),
                black_box(DctSize::Dct32x32),
            )
        });
    });

    group.finish();
}

fn bench_dct_inverse(c: &mut Criterion) {
    let mut group = c.benchmark_group("inverse_dct");

    // 4x4 IDCT
    let input_4x4 = vec![100i16; 16];
    let mut output_4x4 = vec![0i16; 16];
    group.bench_function("4x4", |b| {
        b.iter(|| {
            inverse_dct(
                black_box(&input_4x4),
                black_box(&mut output_4x4),
                black_box(DctSize::Dct4x4),
            )
        });
    });

    // 8x8 IDCT
    let input_8x8 = vec![100i16; 64];
    let mut output_8x8 = vec![0i16; 64];
    group.bench_function("8x8", |b| {
        b.iter(|| {
            inverse_dct(
                black_box(&input_8x8),
                black_box(&mut output_8x8),
                black_box(DctSize::Dct8x8),
            )
        });
    });

    group.finish();
}

fn bench_interpolation(c: &mut Criterion) {
    let mut group = c.benchmark_group("interpolation");

    let width = 64;
    let height = 64;
    let stride = 64;

    // Allocate extra space for filter taps
    let src = vec![128u8; (height + 16) * stride];
    let mut dst = vec![0u8; height * stride];

    // Bilinear
    group.bench_function("bilinear_64x64", |b| {
        b.iter(|| {
            interpolate(
                black_box(&src),
                black_box(&mut dst),
                black_box(stride),
                black_box(stride),
                black_box(width),
                black_box(height),
                black_box(8),
                black_box(8),
                black_box(InterpolationFilter::Bilinear),
            )
        });
    });

    // Bicubic
    group.bench_function("bicubic_64x64", |b| {
        b.iter(|| {
            interpolate(
                black_box(&src),
                black_box(&mut dst),
                black_box(stride),
                black_box(stride),
                black_box(width),
                black_box(height),
                black_box(8),
                black_box(8),
                black_box(InterpolationFilter::Bicubic),
            )
        });
    });

    // 8-tap
    group.bench_function("8tap_64x64", |b| {
        b.iter(|| {
            interpolate(
                black_box(&src),
                black_box(&mut dst),
                black_box(stride),
                black_box(stride),
                black_box(width),
                black_box(height),
                black_box(8),
                black_box(8),
                black_box(InterpolationFilter::EightTap),
            )
        });
    });

    group.finish();
}

fn bench_sad(c: &mut Criterion) {
    let mut group = c.benchmark_group("sad");

    // 16x16 SAD
    let src1_16 = vec![100u8; 16 * 16];
    let src2_16 = vec![110u8; 16 * 16];
    group.bench_function("16x16", |b| {
        b.iter(|| {
            sad(
                black_box(&src1_16),
                black_box(&src2_16),
                black_box(16),
                black_box(16),
                black_box(BlockSize::Block16x16),
            )
        });
    });

    // 32x32 SAD
    let src1_32 = vec![100u8; 32 * 32];
    let src2_32 = vec![110u8; 32 * 32];
    group.bench_function("32x32", |b| {
        b.iter(|| {
            sad(
                black_box(&src1_32),
                black_box(&src2_32),
                black_box(32),
                black_box(32),
                black_box(BlockSize::Block32x32),
            )
        });
    });

    // 64x64 SAD
    let src1_64 = vec![100u8; 64 * 64];
    let src2_64 = vec![110u8; 64 * 64];
    group.bench_function("64x64", |b| {
        b.iter(|| {
            sad(
                black_box(&src1_64),
                black_box(&src2_64),
                black_box(64),
                black_box(64),
                black_box(BlockSize::Block64x64),
            )
        });
    });

    group.finish();
}

fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");
    group.throughput(criterion::Throughput::Bytes((64 * 64) as u64));

    let src1 = vec![100u8; 64 * 64];
    let src2 = vec![110u8; 64 * 64];

    group.bench_function("sad_64x64_throughput", |b| {
        b.iter(|| {
            sad(
                black_box(&src1),
                black_box(&src2),
                black_box(64),
                black_box(64),
                black_box(BlockSize::Block64x64),
            )
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_dct_forward,
    bench_dct_inverse,
    bench_interpolation,
    bench_sad,
    bench_throughput
);

criterion_main!(benches);
