# Presentation Outline

## 1. Problem

How can multiple clients update shared betting state while offline and later converge automatically?

## 2. Why CRDTs?

- Offline-first operation.
- Automatic merge.
- Duplicate and out-of-order message tolerance.
- Eventual convergence without manual conflict dialogs.

## 3. Architecture

```text
Client A --\
Client B ---- Sync Server
Client C --/
```

The server is a transport and operation-log relay. Clients own the CRDT state and conflict handling.

## 4. Implemented CRDTs

- LWW-register for overwrite values.
- OR-set for players, rounds, and bets.
- PN-counter for balance demonstration.
- Bounded counter for spending rights.

## 5. Double-Spend Problem

```text
starting balance: 100
A spends 80 offline
B spends 80 offline
merged PN-counter balance: -60
```

The PN-counter converges, but convergence alone is not enough.

## 6. Bounded-Counter Solution

```text
A rights: 60
B rights: 40
A tries 80 -> rejected
B tries 80 -> rejected
A spends 50, B spends 30 -> accepted
```

Application invariants need CRDTs designed for the invariant.

## 7. Demo

- Run `cargo run -- simulate double-spend`.
- Run `cargo run -- simulate offline-merge`.
- Optionally show two live clients and the server.

## 8. Tests

- CRDT algebraic properties.
- Deduplication.
- Offline merge.
- Out-of-order delivery.
- Double-spend prevention.
- Eventual convergence.

## 9. Limitations

- Fake chips only.
- No authentication.
- No encryption.
- Simplified spending-right transfer model.
- Permissive late-bet rule.
- Not suitable for real gambling.

## 10. Conclusion

BetSync shows both sides of CRDTs: they are powerful for automatic convergence, but application invariants like non-negative balances require additional design.
