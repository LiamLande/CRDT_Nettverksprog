# BetSync

BetSync is a distributed fake-chip betting simulator for demonstrating CRDTs, offline-first operation, operation-log sync, and the limits of eventual consistency.

The main point of the project is not the dice game. The point is to show that ordinary CRDT convergence does not automatically protect application invariants. A PN-counter can converge to the same value on every replica while still allowing an invalid negative chip balance. BetSync demonstrates that failure and then prevents it with a custom bounded counter based on per-replica spending rights.

## Implemented Functionality

- Custom LWW-register, OR-set, PN-counter, and bounded counter implementations.
- Immutable operation model with Lamport timestamps and unique operation IDs.
- Client-side operation log, deduplication, and deterministic state reconstruction.
- Simple JSON-lines TCP sync server that stores and rebroadcasts operations.
- Offline mode with queued local operations and reconnect sync.
- Dice betting with odd/even, high/low, and exact-number bets.
- Round lifecycle: `Created -> BettingOpen -> BettingClosed -> Resolved`.
- Automated tests for CRDT properties, deduplication, offline merge, out-of-order delivery, double-spend prevention, and convergence.

## Architecture

```text
Client A --\
Client B ---- Sync Server
Client C --/
```

The server is deliberately simple. It accepts TCP clients, stores operations in an operation log, sends missing operations when clients reconnect, and broadcasts new operations to connected clients. It does not decide balances, resolve CRDT conflicts, or validate bets.

Each client owns the important distributed logic:

- creates immutable operations
- applies local operations immediately
- stores unsynced operations while offline
- deduplicates remote operations by `op_id`
- rebuilds local CRDT/domain state from the operation log
- validates local spending rights before accepting bets

## CRDT Design

### LWW-Register

Used for simple overwrite values such as table name and player display name.

Merge rule:

```text
highest Lamport timestamp wins
if timestamps are equal, highest replica_id wins
```

Limitation: concurrent edits can overwrite each other, so this is not used for critical money-like state.

### OR-Set

Used for replicated sets such as players, rounds, and bets. Adds create unique tags. Removes only remove tags already observed by that replica, which gives better behavior than a plain replicated set under concurrent add/remove.

### PN-Counter

Used to model basic mergeable balances and to demonstrate the double-spend problem:

```text
starting balance: 100
A offline spends 80
B offline spends 80
merged balance: -60
```

The PN-counter converges correctly, but the application state is invalid.

### Bounded Counter

Used for fake-chip spending rights. Each player has rights assigned per replica:

```text
total chips: 100
A rights: 60
B rights: 40
```

A replica can place a local bet only if it has enough local rights. This prevents offline double-spending because two disconnected replicas cannot spend the same rights.

This is a simplified escrow-style bounded counter. It guarantees that accepted local bets cannot exceed the rights available to that replica. It does not provide authentication, malicious-client protection, or production-grade financial safety.

## Consistency Model

BetSync is eventually consistent. Replicas may temporarily disagree while offline or while messages are delayed, duplicated, or delivered out of order. Once they have seen the same set of operations, they deterministically reconstruct the same state.

Duplicate operations are harmless because every operation has a unique `op_id` and clients keep a `seen_operations` set.

Out-of-order delivery is handled by keeping operations and rebuilding state in deterministic Lamport order. For example, a `PlaceBet` can arrive before `JoinTable`; the replica may temporarily reject it during reconstruction, but once the missing earlier operation arrives, the final reconstructed state converges.

Late bets use a documented CRDT-friendly rule: a bet is accepted if the origin replica had observed `BettingOpen` when the bet was created. This may allow a bet that another replica would have considered late after seeing a close operation. That tradeoff is intentional and documented rather than hidden.

## Installation

Install Rust, then build:

```bash
cargo build
```

Run tests:

```bash
cargo test
```

Generate API docs:

```bash
cargo doc --open
```

## Usage

Start a sync server:

```bash
cargo run -- server --port 8080
```

Start clients in separate terminals:

```bash
cargo run -- client --replica-id A --server 127.0.0.1:8080
cargo run -- client --replica-id B --server 127.0.0.1:8080
```

Useful client commands:

```text
create-table BetSync
join Liam liam
join-split liam Liam A:60 B:40
start-round round-1
bet odd 50
close-round
resolve 5
offline
online
state
log
balance
rights
resend <op_id>
```

Run built-in demo simulations:

```bash
cargo run -- simulate offline-merge
cargo run -- simulate double-spend
```

Start the browser frontend:

```bash
cargo run -- frontend --port 3000
```

Then open:

```text
http://127.0.0.1:3000
```

## Testing

The test suite covers:

- CRDT commutativity, associativity, and idempotence.
- Duplicate bet and payout operations.
- Offline merge between two clients.
- Out-of-order operation delivery.
- PN-counter double-spend failure.
- Bounded-counter double-spend prevention.
- Eventual convergence with different delivery orders and duplicates.

## Project Structure

```text
src/
  client/       local client, offline queue, operation creation
  crdt/         LWW-register, OR-set, PN-counter, bounded counter
  domain/       dice bets, rounds, reconstructed betting state
  server/       sync server and operation store
  sync/         operations, Lamport clock, protocol, operation log
tests/          distributed failure-case and CRDT tests
docs/           demo and presentation notes
```

## External Dependencies

- `clap`: command-line argument parsing.
- `serde` and `serde_json`: JSON serialization for operations and sync messages.
- `uuid`: unique operation and bet IDs.
- `anyhow`: ergonomic CLI error handling.
- `tiny_http`: local HTTP server for the browser demo frontend.

No external CRDT libraries are used. All CRDT logic is implemented in this repository.

## Continuous Integration

The GitHub Actions workflow in `.github/workflows/ci.yml` runs:

```bash
cargo fmt --check
cargo test
```

## Known Limitations

- Fake chips only.
- Simple dice game only.
- No authentication or encryption.
- Trusted sync server transport.
- Simplified bounded counter.
- Late-bet handling is intentionally permissive under offline operation.
- No persistent database.
- Not suitable for real gambling or real-money accounting.

## Future Work

- Persistent operation store.
- Stronger round ownership/dealer rules.
- Cryptographic audit log.
- Peer-to-peer sync.
- Web UI.
- More betting games.

## Sources and External Information

- Project requirements and architecture outline supplied in the assignment prompt.
- Rust standard library documentation for TCP networking, collections, and threading.
- Crate documentation for `clap`, `serde`, `serde_json`, `uuid`, and `anyhow`.

## Key Framing

BetSync demonstrates that CRDTs can provide automatic convergence in an offline-first betting system, but also shows that application invariants such as non-negative balances require additional CRDT design, such as bounded counters.
