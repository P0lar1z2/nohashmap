use nohashmap::ConcurrentMap;
use std::sync::Arc;

fn main() {
    let map: Arc<ConcurrentMap<String>> = Arc::new(ConcurrentMap::new());
    // 这里只做简单演示，实际并发需用线程
    map.write_init();
    map.insert(42, "hello".to_string());
    map.write_done();

    map.read_init();
    let v = map.get(42, |v| v.cloned());
    println!("value: {:?}", v);
    map.read_done();
}
