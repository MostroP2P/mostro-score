# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Query-level event filtering using `.custom_tag()` to only fetch events with `z=order` tag
- Required imports: `Alphabet` and `SingleLetterTag` from nostr-sdk
- Debug logging to display status distribution for order events
- Sample event output showing first 3 events with full tag structure
- Implementation note in spec file documenting v1 focuses on orders only

### Changed
- **BREAKING**: Filter now uses query-level filtering instead of client-side filtering
  - Events are filtered at the relay level using `#z=["order"]` tag filter
  - Significantly reduces bandwidth and improves performance
- Simplified event processing logic by removing z-tag validation
- Updated trust score calculation to remove dispute penalties
- Updated `specs/reputation_system_v1.md` to reflect query-level filtering approach
- Simplified debug output to focus on order-specific metrics

### Removed
- **BREAKING**: Dispute tracking functionality
  - Removed `disputes_opened` field from `MostroStats` struct
  - Removed dispute event counting and output
  - Removed dispute penalty from trust score calculation (was -10 points per dispute)
  - Removed "Dispute Rate" and "Risk Level" metrics from spec
- Client-side z-tag filtering and validation logic (42 lines)
- Dead code warnings:
  - Removed unused `total_events_seen: usize` field from `MostroStats`
  - Removed unused `unique_users: HashSet<String>` field from `MostroStats`
  - Removed unused `HashSet` import
- Debug tracking variables:
  - Removed `z_tag_distribution` HashMap
  - Removed `orders_with_z_tag` counter
- Z-tag distribution output from debug information
- Dispute-based scam detection heuristic from spec requirements

### Fixed
- Dead code compiler warnings (2 unused struct fields)
- Improved event filtering efficiency by moving from client-side to query-level filtering

### Performance
- Reduced network bandwidth by only fetching order events instead of all kind 38383 events
- Reduced memory usage by not loading dispute, info, and other non-order event types
- Faster event processing due to fewer events to analyze

## Technical Details

### Filter Changes
**Before:**
```rust
let filter = Filter::new()
    .kind(Kind::Custom(38383))
    .author(public_key);
```

**After:**
```rust
let filter = Filter::new()
    .kind(Kind::Custom(38383))
    .author(public_key)
    .custom_tag(SingleLetterTag::lowercase(Alphabet::Z), vec!["order"]);
```

This generates a Nostr filter:
```json
{
  "kinds": [38383],
  "authors": ["<pubkey>"],
  "#z": ["order"]
}
```

### Trust Score Calculation Changes
**Before:**
- Age: 0-30 points (max at 1 year)
- Volume: 0-40 points (max at 1 BTC)
- Success Count: 0-30 points (max at 100 orders)
- Penalties: -10 points per dispute

**After:**
- Age: 0-30 points (max at 1 year)
- Volume: 0-40 points (max at 1 BTC)
- Success Count: 0-30 points (max at 100 orders)
- No penalties

### Migration Notes
If you were using the dispute tracking feature, note that:
- Dispute events are no longer fetched or analyzed
- Trust scores will be higher since dispute penalties are removed
- To re-enable dispute tracking in the future, a separate query with `z=dispute` filter would be needed

### Default Relay Change
- Changed default relay from multiple relays to `wss://relay.mostro.network` for better Mostro protocol coverage
