use betsync::{BetKind, LocalClient, OperationKind};

#[test]
fn place_bet_can_arrive_before_join_and_still_converge() {
    let mut source = LocalClient::new("A");
    source.create_table("Out of order");
    source.join_table("liam", "Liam", 100);
    source.start_round("round-1", "dealer");
    source
        .place_bet("liam", "round-1", 40, BetKind::High)
        .unwrap();

    let operations = source.log.sorted_operations();
    let bet = operations
        .iter()
        .find(|operation| matches!(operation.kind, OperationKind::PlaceBet { .. }))
        .unwrap()
        .clone();

    let mut replica = LocalClient::new("B");
    replica.ingest(bet.clone());
    assert!(replica.state.rejected_operations.contains_key(&bet.op_id));

    for operation in operations {
        if operation.op_id != bet.op_id {
            replica.ingest(operation);
        }
    }

    assert_eq!(replica.state, source.state);
    assert_eq!(replica.state.visible_bets().len(), 1);
}
