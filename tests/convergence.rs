use betsync::{BetKind, LocalClient};

#[test]
fn replicas_converge_with_different_delivery_orders_and_duplicates() {
    let mut source = LocalClient::new("A");
    source.create_table("Convergence");
    source.join_table("liam", "Liam", 100);
    source.start_round("round-1", "dealer");
    source
        .place_bet("liam", "round-1", 25, BetKind::Low)
        .unwrap();
    source.close_betting("round-1").unwrap();
    source.resolve_round("round-1", 2).unwrap();

    let operations = source.log.sorted_operations();
    let mut replica_a = LocalClient::new("R1");
    let mut replica_b = LocalClient::new("R2");
    let mut replica_c = LocalClient::new("R3");
    assert_eq!(operations.len(), 8);

    replica_a.ingest_many(operations.clone());

    for index in [2, 0, 4, 1, 3, 5, 7, 6] {
        replica_b.ingest(operations[index].clone());
    }

    for index in [1, 1, 7, 6, 3, 0, 5, 2, 4] {
        replica_c.ingest(operations[index].clone());
    }

    assert_eq!(replica_a.state, source.state);
    assert_eq!(replica_b.state, source.state);
    assert_eq!(replica_c.state, source.state);
}
