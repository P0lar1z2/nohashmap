use criterion::{Criterion, criterion_group, criterion_main};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use nohashmap::ConcurrentMap;

const N: usize = 10_000_000;
const THREADS: [usize; 7] = [1, 2, 4, 8, 12, 16, 32];

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
            for h in handles {
                let _ = h.join();
            }
            let times = thread_times.lock().unwrap();
            
            // 计算每个线程的QPS和总QPS
            let mut total_qps = 0.0;
            for (i, micros) in times.iter().enumerate() {
                let seconds = *micros as f64 / 1_000_000.0; // 转换为秒
                let thread_qps = (N - 1) as f64 / seconds; // 每个线程的QPS
                total_qps += thread_qps;
                println!("Thread {i}: {:.3} ms, QPS: {:.2}", *micros as f64 / 1000.0, thread_qps);
            }
            println!("Total QPS: {:.2}", total_qps);
            
            // 返回最大耗时作为 Criterion 的采样
            let max = times.iter().max().copied().unwrap_or(0);
            Duration::from_micros(max as u64)
        });
    });
    group.finish();
}

pub fn criterion_benchmark(c: &mut Criterion) {
    for &threads in &THREADS {
        bench_concurrent_read_dataset(threads, c);
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
