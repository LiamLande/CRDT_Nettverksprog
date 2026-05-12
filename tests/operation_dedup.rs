use betsync::{BetKind, LocalClient, OperationKind};

#[test]
fn duplicate_bet_operation_does_not_double_spend() {
    let mut client = LocalClient::new("A");
    client.create_table("Dedup");
    client.join_table("liam", "Liam", 100);
    client.start_round("round-1", "dealer");

    let bet = client
        .place_bet("liam", "round-1", 50, BetKind::Odd)
        .unwrap();
    let balance_after_once = client.state.player_balance("liam");
    let rights_after_once = client.state.available_rights("liam", "A");

    assert!(!client.ingest(bet));
    assert_eq!(client.state.player_balance("liam"), balance_after_once);
    assert_eq!(
        client.state.available_rights("liam", "A"),
        rights_after_once
    );
    assert_eq!(client.state.visible_bets().len(), 1);
}

#[test]
fn duplicate_payout_operation_does_not_pay_twice() {
    let mut client = LocalClient::new("A");
    client.create_table("Dedup");
    client.join_table("liam", "Liam", 100);
    client.start_round("round-1", "dealer");
    client
        .place_bet("liam", "round-1", 50, BetKind::Odd)
        .unwrap();
    client.close_betting("round-1").unwrap();
    let operations = client.resolve_round("round-1", 5).unwrap();
    let payout = operations
        .into_iter()
        .find(|operation| matches!(operation.kind, OperationKind::ApplyPayout { .. }))
        .unwrap();

    assert_eq!(client.state.player_balance("liam"), 150);
    assert!(!client.ingest(payout));
    assert_eq!(client.state.player_balance("liam"), 150);
}
