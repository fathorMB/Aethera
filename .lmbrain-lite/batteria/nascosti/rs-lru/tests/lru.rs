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

#[test]
fn hidden_capacity_zero_stores_nothing() {
    let mut c = LruCache::new(0);
    assert_eq!(c.put(1, "x"), None);
    assert!(c.is_empty());
    assert_eq!(c.get(&1), None);
    assert!(c.keys_by_recency().is_empty());
}

#[test]
fn hidden_missing_key_does_not_change_order() {
    let mut c = LruCache::new(3);
    c.put(1, 1);
    c.put(2, 2);
    assert_eq!(c.get(&9), None);
    assert_eq!(c.keys_by_recency(), vec![1, 2]);
}

#[test]
fn hidden_peek_keeps_order_and_long_sequence() {
    let mut c = LruCache::new(3);
    for i in 0..10 {
        c.put(i, i * 10);
        assert!(c.len() <= 3);
    }
    assert_eq!(c.keys_by_recency(), vec![7, 8, 9]);
    assert_eq!(c.peek(&7), Some(&70));
    assert_eq!(c.keys_by_recency(), vec![7, 8, 9]);
    assert_eq!(c.get(&7), Some(&70));
    assert_eq!(c.keys_by_recency(), vec![8, 9, 7]);
    assert_eq!(c.put(10, 100), Some((8, 80)));
    assert_eq!(c.remove(&9), Some(90));
    assert_eq!(c.put(11, 110), None);
    assert_eq!(c.keys_by_recency(), vec![7, 10, 11]);
    assert_eq!(c.len(), 3);
}

#[test]
fn hidden_capacity_one() {
    let mut c = LruCache::new(1);
    c.put("a", 1);
    assert_eq!(c.put("b", 2), Some(("a", 1)));
    assert_eq!(c.put("b", 3), None);
    assert_eq!(c.get(&"b"), Some(&3));
    assert_eq!(c.len(), 1);
}
