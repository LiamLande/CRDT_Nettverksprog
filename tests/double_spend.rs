use betsync::crdt::PNCounter;
use betsync::{BetKind, LocalClient};
use std::collections::BTreeMap;

#[test]
fn pn_counter_converges_but_can_produce_negative_balance() {
    let mut base = PNCounter::default();
    base.increment("bank", 100);

    let mut replica_a = base.clone();
    let mut replica_b = base;
    replica_a.decrement("A", 80);
    replica_b.decrement("B", 80);

    let merged = replica_a.merge(&replica_b);
    assert_eq!(merged.value(), -60);
}

#[test]
fn bounded_spending_rights_prevent_offline_double_spend() {
    let mut rights = BTreeMap::new();
    rights.insert("A".to_string(), 60);
    rights.insert("B".to_string(), 40);

    let mut a = LocalClient::new("A");
    a.create_table("Bounded");
    a.join_table_with_rights("liam", "Liam", 100, rights);
    a.start_round("round-1", "dealer");

    let mut b = LocalClient::new("B");
    b.ingest_many(a.log.operations.clone());

    assert!(a.place_bet("liam", "round-1", 80, BetKind::Odd).is_err());
    assert!(b.place_bet("liam", "round-1", 80, BetKind::Even).is_err());

    a.place_bet("liam", "round-1", 50, BetKind::Odd).unwrap();
    b.place_bet("liam", "round-1", 30, BetKind::Even).unwrap();

    a.ingest_many(b.log.operations.clone());
    b.ingest_many(a.log.operations.clone());

    assert_eq!(a.state, b.state);
    assert_eq!(a.state.player_balance("liam"), 20);
    assert!(a.state.player_balance("liam") >= 0);
}
