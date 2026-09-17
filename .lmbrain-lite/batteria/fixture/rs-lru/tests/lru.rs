use lru::LruCache;

#[test]
fn stores_and_reads() {
    let mut c = LruCache::new(2);
    assert_eq!(c.put("a", 1), None);
    assert_eq!(c.get(&"a"), Some(&1));
    assert_eq!(c.len(), 1);
}

#[test]
fn evicts_least_recently_used() {
    let mut c = LruCache::new(2);
    c.put("a", 1);
    c.put("b", 2);
    c.get(&"a");
    assert_eq!(c.put("c", 3), Some(("b", 2)));
    assert_eq!(c.keys_by_recency(), vec!["a", "c"]);
}

#[test]
fn update_moves_to_back_without_evicting() {
    let mut c = LruCache::new(2);
    c.put("a", 1);
    c.put("b", 2);
    assert_eq!(c.put("a", 10), None);
    assert_eq!(c.keys_by_recency(), vec!["b", "a"]);
    assert_eq!(c.peek(&"a"), Some(&10));
}
