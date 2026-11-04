# Player 2 Character Spawning Implementation Research

## Task Overview
Implement player 2 character spawning in the Skate 2 multiplayer co-op system. When player 2 presses START, they should appear in the game world with independent input/camera/physics.

## Current State
- `TwoPlayerSession` is created and `activate_player2(char_id)` is called when P2 presses START
- Session transitions to `BothActive` state
- Character ID is stored (currently placeholder 1)
- No actual spawning of P2's actor yet occurs

## Character Instantiation Pipeline

### 1. Player 1 Initialization
Player 1's character spawning follows this flow:

#### Bootstrap Path
- File: `crates/skate2-core/src/session/selected_session_public_facade.rs`
- Flow: `try_begin_controlled()` → loads player 1
- Key struct: `SelectedSession` holds `self.inner.player: Option<SelectedPlayerPartition>`

#### Spawning During Gameplay
- File: `crates/skate2-core/src/session/selected_session_freeskate.rs` line 368
- Method: `apply_pending_character()` → checks if player 1's character is loaded
- Then: `apply_spawn_character_attributes()` (line 385) → applies spawn-time attributes
- Method: `replace_local_character()` (line 456) → actually replaces/loads character

### 2. Party Play Model (Multi-player Pattern)
Party Play shows the correct pattern for spawning additional actors:

#### Core Implementation
- File: `crates/skate2-core/src/session/selected_session_party_skaters.rs`
- Key method: `try_build_later_skater()` line 308-530

#### What This Method Does
```
try_build_later_skater(player: u32, character, spawn: [f32; 16], prepared)
  ↓
  1. Get player's configuration (peer_id, loader, recipe_load)
  2. Load character recipe (Marquee or Create-A-Character)
  3. Create `ActorConstruction::new()` with:
     - input_player: the player's input handler
     - authority: physical world manager
     - actor_flags: behavior flags
     - spawn: [f32; 16] transform matrix at spawn location
  ↓
  4. Build actor prefix:
     - try_construct_prefix() - setup chase formation
     - try_construct_listener() - attach input listener
     - try_construct_graphs() - load animation graphs
  ↓
  5. Build animation system:
     - try_complete_player_animation() - full animation pipeline
     - try_construct_breadcrumbs_from_animation()
  ↓
  6. Build physics:
     - try_construct_selected_physical_pair() - world simulation integration
  ↓
  7. Result: PartySkater struct with fully constructed actor
```

### 3. Data Structures Needed

#### ActorConstruction (line 383 in actor_construction.rs)
- Holds: animation listener, graphs, shared state
- Created with: `ActorConstruction::new(input_player, authority, actor_flags, spawn_transform)`

#### SelectedPlayerPartition
- Holds: first player's data
- Contains: `selected: SelectedPlayer` with actor, animation, board
- File: see references in selected_session_public_facade.rs

#### Spawn Transform
- Type: `[f32; 16]` (4x4 matrix)
- Helper: `SkeletonRootMatrix::known(spawn)` creates properly formatted transform
- Location: same as player 1, or slightly offset

#### Player Input
- Type: `Arc<GameInputPlayer>`
- From: `selected.input.gameplay.player(player as usize)`
- Direction: P2 should use input slot 1 (second controller)

#### Formation & Authority
- Chase formation: `Arc<Mutex<ActorChaseFormationOwner>>`
- Authority: World's `physical_manager_authority()` for physics integration

## Key Files & APIs

### Session Management
- **File**: `crates/skate2-core/src/session/selected_session_public_facade.rs`
- **API**: `self.session.two_player_mut().activate_player2(char_id)`
- **Usage**: Called from live_skating_client.rs line 2635 when START pressed

### Two Player Session
- **File**: `crates/skate2-core/src/session/two_player_session.rs`
- **Struct**: `TwoPlayerSession { state, objective, player2_character_id }`
- **API**: `activate_player2(char_id)` - sets state to BothActive

### Character Loading
- **File**: `crates/skate2-core/src/session/selected_session_freeskate.rs`
- **API**: `apply_pending_character()` - loads queued character
- **API**: `apply_spawn_character_attributes()` - applies spawn-time attributes

### Actor Construction
- **File**: `crates/skate2-core/src/actors/actor_construction.rs` line 383
- **API**: `ActorConstruction::new(input, authority, flags, spawn)`
- **Methods**:
  - `try_construct_prefix()` - setup phase
  - `try_construct_listener()` - input binding
  - `try_construct_graphs()` - animation
  - `try_complete_player_animation()` - full animation
  - `try_construct_breadcrumbs_from_animation()` - state tracking

### Party Spawn Pattern (Reference Implementation)
- **File**: `crates/skate2-core/src/session/selected_session_party_skaters.rs`
- **Method**: `try_build_later_skater()` line 308
- **Result**: Returns fully constructed `PartySkater` with independent actor

### Spawn Location
- **File**: `apps/skate2-pc/src/live_skating_client.rs` line 25
- **Function**: `selected_spawn_location()` - gets default spawn
- **API**: From `crate::player` module

### Input Routing
- **File**: `crates/skate2-core/src/input/two_player_input_router.rs`
- **Status**: Already routes input to P2
- **Note**: P2 input comes from controller slot 1

## Implementation Strategy

### Phase 1: Hook activate_player2() to trigger spawning
- In `selected_session_public_facade.rs`, enhance `try_tick_once()`
- When `two_player.state == BothActive` and player2_character_id is set:
  - Check if P2 character is not already loaded (similar to party_play pattern)
  - Queue P2 character for loading

### Phase 2: Load P2's character recipe
- Adapt pattern from `try_build_later_skater()` in party_skaters.rs
- Get P2's character ID from `two_player.player2_character_id`
- Load same character as P1 (duplicate instantiation initially)
- Prepare recipe and loader state

### Phase 3: Spawn P2's actor
- Create `ActorConstruction` for P2 using:
  - Player index: 1 (input slot 1)
  - Spawn: same location as P1 (or slightly offset)
  - Character: loaded recipe
- Build full construction pipeline:
  1. Prefix + listener + graphs
  2. Animation
  3. Physics integration
  4. State tracking (breadcrumbs)

### Phase 4: Store in session structure
- Add `player2_actor` field to `SelectedSession` or `TwoPlayerSession`
- Ensure P2 actor is updated alongside P1 in each tick
- Sync P2 input/camera/physics independently

### Phase 5: Integrate into frame update
- In `try_tick_once()`, when updating P1's frame:
  - Also update P2's frame in parallel
  - P2 receives its own input from P2 controller
  - P2 has independent physics/animation state

## Blockers & Unknowns

### Confirmed
- ✅ Input routing works (P2 controller feeds into slot 1)
- ✅ Party Play shows multi-actor pattern works
- ✅ Spawn location API available
- ✅ Actor construction is mature and tested

### To Investigate
- [ ] Where should P2 actor be stored? In `TwoPlayerSession` or `SelectedSession`?
- [ ] Does P2 share world simulation or get independent physics?
- [ ] How to handle character selection later (don't use same char)?
- [ ] Should P2 spawn at same location as P1 or offset?
- [ ] How to handle camera for P2? Split-screen planning needed?

### Dependencies
- All required structs/APIs already exist
- No new data formats needed (same as Party Play)
- Actor construction pipeline is proven

## References

### Party Play Multi-Player Implementation
- File: `crates/skate2-core/src/session/selected_session_party_skaters.rs`
- Shows: Complete multi-actor spawning pattern
- Pattern: Adapt `try_build_later_skater()` for P2 spawning

### Original Player Bootstrap
- File: `docs/evidence/selected-session-player-startup-draft-2026-09-23.md`
- Shows: First player initialization in detail
- Use: As reference for P2 sequence

### Input Routing (Already Done)
- File: `crates/skate2-core/src/input/two_player_input_router.rs`
- Status: P2 input properly routed to controller slot 1

## Implementation Status

### Phase 1: COMPLETED
- ✅ Added `player2_spawn_pending` flag to `TwoPlayerSession`
- ✅ Set flag when `activate_player2()` is called
- ✅ Added `try_spawn_player2()` method to freeskate session
- ✅ Hooked into `try_tick()` after `apply_pending_character()`
- ✅ Placeholder implementation that detects when P2 should spawn
- ✅ Full build succeeds (skate2-pc binary compiles successfully)

### Current Implementation
The infrastructure is now in place:
1. When P2 presses START: `activate_player2(char_id)` sets `player2_spawn_pending = true`
2. During each `try_tick()`: `try_spawn_player2()` is called
3. Method checks if P1 is loaded and P2 spawn is pending
4. Currently logs that spawn is requested but defers full implementation

### Phase 2: Full Actor Construction (PENDING)
The actual P2 character spawning requires:

1. **Character Recipe Loading**
   - Load P2's marquee character definition
   - Use party_skaters machinery or adapt for P2

2. **Actor Construction (Key Blocker)**
   - `ActorConstruction::new()` needs initialization
   - Challenge: SelectedSession<T,C> generic T constraint for try_spawn_party_skater
   - Solution: Implement dedicated P2 spawning instead of reusing party machinery

3. **Storage Integration**
   - Add P2 actor to session state (either TwoPlayerSession or as Vec in SelectedSession)
   - Ensure frame building includes P2 in "others" list

4. **Physics & Animation**
   - Build P2's physical pair
   - Bind P2 to animation system
   - Sync P2 input from controller slot 1

5. **Camera & Rendering**
   - Implement P2 camera (initially same as P1, later split-screen)
   - Ensure P2 is rendered in frame

### Technical Challenges Resolved

**Generic T Constraint Issue:**
- The try_spawn_party_skater method has a where clause: `T: FnMut() -> Result<u64, E>`
- This constraint couldn't be added to freeskate methods because T is unconstrained in the impl block
- Solution: Instead of using party machinery, implement direct P2 spawning
- This gives full control and avoids complex generic trait bounds

### Key Files Modified
1. `crates/skate2-core/src/session/two_player_session.rs`
   - Added `player2_spawn_pending: bool` field
   - Updated `activate_player2()` to set flag
   - Updated `complete_remove_player2()` to clear flag
   - Updated tests

2. `crates/skate2-core/src/session/selected_session_freeskate.rs`
   - Added `try_spawn_player2()` method
   - Integrated into `try_tick()` flow

3. `crates/skate2-core/src/session/selected_session_public_facade.rs`
   - Call `try_spawn_player2()` after `apply_pending_character()`

## Next Steps

1. Implement dedicated P2 actor construction (avoid party machinery)
2. Create P2-specific structs or extend existing structures to hold P2 data
3. Build full animation/physics pipeline for P2
4. Test with both controllers active
5. Verify independent input/physics/animation state
6. Implement split-screen camera (later phase)
