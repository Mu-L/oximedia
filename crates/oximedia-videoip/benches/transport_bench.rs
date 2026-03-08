//! Transport layer benchmarks.

use bytes::Bytes;
use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use oximedia_videoip::packet::PacketBuilder;
use oximedia_videoip::transport::UdpTransport;
use oximedia_videoip::types::StreamType;
use std::hint::black_box;
use std::net::SocketAddr;
use tokio::runtime::Runtime;

fn bench_packet_encoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("packet_encoding");

    for size in [1024, 4096, 8192] {
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_function(format!("encode_{}", size), |b| {
            let payload = Bytes::from(vec![0u8; size]);

            b.iter(|| {
                let packet = PacketBuilder::new(0)
                    .video()
                    .keyframe()
                    .with_timestamp(12345)
                    .with_stream_type(StreamType::Program)
                    .build(payload.clone())
                    .unwrap();

                black_box(packet.encode())
            });
        });
    }

    group.finish();
}

fn bench_packet_decoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("packet_decoding");

    for size in [1024, 4096, 8192] {
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_function(format!("decode_{}", size), |b| {
            let payload = Bytes::from(vec![0u8; size]);
            let packet = PacketBuilder::new(0).video().build(payload).unwrap();

            let encoded = packet.encode();

            b.iter(|| {
                let decoded = oximedia_videoip::packet::Packet::decode(&encoded[..]).unwrap();
                black_box(decoded)
            });
        });
    }

    group.finish();
}

fn bench_udp_send_receive(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("udp_transport");

    for size in [1024, 4096] {
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_function(format!("send_receive_{}", size), |b| {
            b.iter(|| {
                rt.block_on(async {
                    let addr1: SocketAddr = "127.0.0.1:0".parse().unwrap();
                    let addr2: SocketAddr = "127.0.0.1:0".parse().unwrap();

                    let mut transport1 = UdpTransport::bind(addr1).await.unwrap();
                    let mut transport2 = UdpTransport::bind(addr2).await.unwrap();

                    let packet = PacketBuilder::new(0)
                        .video()
                        .build(Bytes::from(vec![0u8; size]))
                        .unwrap();

                    let dest = transport2.local_addr();
                    transport1.send_packet(&packet, dest).await.unwrap();

                    let (received, _) = transport2.recv_packet().await.unwrap();
                    black_box(received)
                })
            });
        });
    }

    group.finish();
}

fn bench_jitter_buffer(c: &mut Criterion) {
    let mut group = c.benchmark_group("jitter_buffer");

    group.bench_function("add_packet", |b| {
        let mut buffer = oximedia_videoip::jitter::JitterBuffer::new(1000, 20);

        b.iter(|| {
            let packet = PacketBuilder::new(0)
                .video()
                .build(Bytes::from_static(b"test"))
                .unwrap();

            buffer.add_packet(packet).unwrap();
            buffer.clear();
        });
    });

    group.bench_function("get_packet", |b| {
        let mut buffer = oximedia_videoip::jitter::JitterBuffer::new(1000, 0);

        // Pre-fill buffer
        for i in 0..100 {
            let packet = PacketBuilder::new(i)
                .video()
                .build(Bytes::from_static(b"test"))
                .unwrap();
            buffer.add_packet(packet).unwrap();
        }

        b.iter(|| {
            let packet = buffer.get_packet_immediate();
            black_box(packet)
        });
    });

    group.finish();
}

fn bench_fec_encoding(c: &mut Criterion) {
    use oximedia_videoip::fec::FecEncoder;

    let mut group = c.benchmark_group("fec");

    group.bench_function("encode_10_2", |b| {
        let encoder = FecEncoder::new(10, 2).unwrap();

        let packets: Vec<_> = (0..10)
            .map(|i| {
                PacketBuilder::new(i)
                    .video()
                    .build(Bytes::from(vec![0u8; 1000]))
                    .unwrap()
            })
            .collect();

        b.iter(|| {
            let parity = encoder
                .encode(&packets, 100, 12345, StreamType::Program)
                .unwrap();
            black_box(parity)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_packet_encoding,
    bench_packet_decoding,
    bench_udp_send_receive,
    bench_jitter_buffer,
    bench_fec_encoding
);
criterion_main!(benches);
