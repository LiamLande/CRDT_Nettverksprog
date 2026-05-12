use betsync::{BetKind, LocalClient};
use std::collections::BTreeMap;

#[test]
fn offline_clients_merge_to_the_same_state() {
    let mut rights = BTreeMap::new();
    rights.insert("A".to_string(), 60);
    rights.insert("B".to_string(), 40);

    let mut a = LocalClient::new("A");
    a.create_table("Offline Merge");
    a.join_table_with_rights("liam", "Liam", 100, rights);
    a.start_round("round-1", "dealer");

    let mut b = LocalClient::new("B");
    b.ingest_many(a.log.operations.clone());

    a.set_online(false);
    b.set_online(false);
    a.place_bet("liam", "round-1", 50, BetKind::Odd).unwrap();
    b.place_bet("liam", "round-1", 30, BetKind::Even).unwrap();

    a.ingest_many(b.log.operations.clone());
    b.ingest_many(a.log.operations.clone());

    assert_eq!(a.state, b.state);
    assert_eq!(a.state.visible_bets().len(), 2);
    assert_eq!(a.state.player_balance("liam"), 20);
    assert_eq!(a.state.available_rights("liam", "A"), 10);
    assert_eq!(a.state.available_rights("liam", "B"), 10);
}
