use betsync::crdt::{BoundedCounter, LwwRegister, OrSet, PNCounter};

#[test]
fn lww_register_is_commutative_associative_and_idempotent() {
    let a = LwwRegister::new("alpha".to_string(), 1, "A");
    let b = LwwRegister::new("bravo".to_string(), 2, "B");
    let c = LwwRegister::new("charlie".to_string(), 2, "C");

    assert_eq!(a.merge(&b), b.merge(&a));
    assert_eq!(a.merge(&b).merge(&c), a.merge(&b.merge(&c)));
    assert_eq!(c.merge(&c), c);
    assert_eq!(a.merge(&b).merge(&c).value, "charlie");
}

#[test]
fn or_set_is_commutative_associative_and_idempotent() {
    let alpha = "alpha".to_string();
    let beta = "beta".to_string();

    let mut a = OrSet::default();
    a.add(alpha.clone(), "A-1");

    let mut b = a.clone();
    b.remove_observed(&alpha, "B-1");

    let mut c = OrSet::default();
    c.add(beta.clone(), "C-1");

    assert_eq!(a.merge(&b), b.merge(&a));
    assert_eq!(a.merge(&b).merge(&c), a.merge(&b.merge(&c)));
    assert_eq!(c.merge(&c), c);
    assert!(!a.merge(&b).contains(&alpha));
    assert!(a.merge(&b).merge(&c).contains(&beta));
}

#[test]
fn pn_counter_is_commutative_associative_and_idempotent() {
    let mut a = PNCounter::default();
    a.increment("A", 10);
    a.decrement("A", 3);

    let mut b = PNCounter::default();
    b.increment("B", 4);
    b.decrement("B", 2);

    let mut c = PNCounter::default();
    c.increment("A", 15);

    assert_eq!(a.merge(&b), b.merge(&a));
    assert_eq!(a.merge(&b).merge(&c), a.merge(&b.merge(&c)));
    assert_eq!(c.merge(&c), c);
    assert_eq!(a.merge(&b).merge(&c).value(), 14);
}

#[test]
fn bounded_counter_merge_is_commutative_associative_and_idempotent() {
    let mut a = BoundedCounter::default();
    a.set_initial_rights("A", 60);
    a.set_initial_rights("B", 40);
    assert!(a.spend("spend-A", "A", 50));

    let mut b = BoundedCounter::default();
    b.set_initial_rights("A", 60);
    b.set_initial_rights("B", 40);
    assert!(b.spend("spend-B", "B", 30));

    let mut c = BoundedCounter::default();
    c.set_initial_rights("A", 60);
    c.set_initial_rights("B", 40);
    c.grant("payout-A", "A", 20);

    assert_eq!(a.merge(&b), b.merge(&a));
    assert_eq!(a.merge(&b).merge(&c), a.merge(&b.merge(&c)));
    assert_eq!(c.merge(&c), c);

    let merged = a.merge(&b).merge(&c);
    assert_eq!(merged.available("A"), 30);
    assert_eq!(merged.available("B"), 10);
}
