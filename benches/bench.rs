use criterion::{Criterion, criterion_group, criterion_main};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex};

use nohashmap::ConcurrentMap;

const N: usize = 10_000_000;
const THREADS: [usize; 5] = [4, 8, 12, 16, 32];

fn setup_map() -> Arc<ConcurrentMap<u64>> {
    let map = Arc::new(ConcurrentMap::new());
    map.write_init();
    for i in 0..N as u64 {
        map.insert(i, i);
    }
    map.write_done();
    println!("[BENCH] setup_map: done");
    map
}

// fn bench_concurrent_read(threads: usize, c: &mut Criterion) {
//     println!("[BENCH] bench_concurrent_read: {} threads", threads);
//     let map = setup_map(); // 只构建一次
//     let mut group = c.benchmark_group(format!("read_{}threads", threads));
//     group.sample_size(10);
//     group.measurement_time(Duration::from_secs(10));
//     group.bench_function("concurrent_read", |b| {
//         b.iter(|| {
//             // println!("[BENCH] iter: {} threads", threads);
//             let mut handles = Vec::new();
//             for t in 0..threads {
//                 let map = map.clone();
//                 handles.push(thread::spawn(move || {
//                     let chunk = N / threads;
//                     let start = t * chunk;
//                     let end = if t == threads - 1 { N } else { (t + 1) * chunk };
//                     map.read_init();
//                     let mut sum = 0u64;
//                     for i in start..end {
//                         sum += map.get(i as u64, |v| v.copied().unwrap_or(0));
//                     }
//                     map.read_done();
//                     sum
//                 }));
//             }
//             let total: u64 = handles.into_iter().map(|h| h.join().unwrap()).sum();
//             assert_eq!(total, (N as u64 - 1) * (N as u64) / 2);
//         });
//     });
//     group.finish();
// }

fn bench_concurrent_read_count(threads: usize, c: &mut Criterion) {
    println!("[BENCH] bench_concurrent_read_count: {} threads", threads);
    let map = setup_map();
    let mut group = c.benchmark_group(format!("read_count_{}threads", threads));
    group.sample_size(10);
    group.bench_function("concurrent_read_count", |b| {
        b.iter_custom(|_duration| {
            let running = Arc::new(AtomicBool::new(true));
            let mut handles = Vec::new();
            let counters: Vec<_> = (0..threads).map(|_| Arc::new(AtomicU64::new(0))).collect();
            let start = Instant::now();
            for t in 0..threads {
                let map = map.clone();
                let running = running.clone();
                let counter = counters[t].clone();
                handles.push(thread::spawn(move || {
                    let mut idx = 1;
                    map.read_init();
                    while running.load(Ordering::Relaxed) {
                        let i = idx as u64;
                        let _ = map.get(i, |v| v.copied().unwrap_or(0));
                        counter.fetch_add(1, Ordering::Relaxed);
                        idx += 1;
                        if idx >= N { idx = 1; }
                    }
                    map.read_done();
                }));
            }
            // 运行10秒
            std::thread::sleep(Duration::from_secs(10));
            running.store(false, Ordering::Relaxed);
            for h in handles { let _ = h.join(); }
            let elapsed = start.elapsed();
            let counts: Vec<u64> = counters.iter().map(|c| c.load(Ordering::Relaxed)).collect();
            let total: u64 = counts.iter().sum();
            for (i, c) in counts.iter().enumerate() {
                println!("Thread {i}: {c} ops");
            }
            println!("Total: {total} ops in {:.2?} ({:.2} ops/s)", elapsed, total as f64 / elapsed.as_secs_f64());
            // 返回 Criterion 需要的 Duration
            elapsed
        });
    });
    group.finish();
}

fn bench_concurrent_read_dataset(threads: usize, c: &mut Criterion) {
    println!("[BENCH] bench_concurrent_read_dataset: {} threads", threads);
    let map = setup_map();
    let mut group = c.benchmark_group(format!("read_dataset_{}threads", threads));
    group.sample_size(10);
    group.bench_function("concurrent_read_dataset", |b| {
        b.iter_custom(|_duration| {
            let thread_times = Arc::new(Mutex::new(vec![0u128; threads]));
            let mut handles = Vec::new();
            for t in 0..threads {
                let map = map.clone();
                let thread_times = thread_times.clone();
                handles.push(std::thread::spawn(move || {
                    let start = Instant::now();
                    map.read_init();
                    for i in 1..N {
                        let _ = map.get(i as u64, |v| v.copied().unwrap_or(0));
                    }
                    map.read_done();
                    let elapsed = start.elapsed();
                    let mut times = thread_times.lock().unwrap();
                    times[t] = elapsed.as_micros();
                }));
            }
            for h in handles { let _ = h.join(); }
            let times = thread_times.lock().unwrap();
            for (i, micros) in times.iter().enumerate() {
                println!("Thread {i}: {:.3} ms", *micros as f64 / 1000.0);
            }
            // 返回最大耗时作为 Criterion 的采样
            let max = times.iter().max().copied().unwrap_or(0);
            Duration::from_micros(max as u64)
        });
    });
    group.finish();
}

pub fn criterion_benchmark(c: &mut Criterion) {
    for &threads in &THREADS {
        // bench_concurrent_read(threads, c);
        // bench_concurrent_read_count(threads, c);
        bench_concurrent_read_dataset(threads, c);
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
