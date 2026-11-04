# Local split-screen co-op (PC extension)

The Xbox 360 reference build has no local split-screen free skate: offline it follows one local
skater with one camera, and its only local multiplayer is Party Play, whose players take turns.
Everything on this page is a **PC-only extension**. None of it is reconstructed reference
behaviour or carries a parity confidence. It is built from reference pieces (Party Play's
skater construction, `PlayerUI` focus, `Actor::SetTransform`, the trick HUD's scoring frame,
the stereo renderer's passes) and keeps every single-player path unchanged while the second
player is not joined.

## Playing

| Action | Pad | Result |
| --- | --- | --- |
| Join | pad 1 (the second connected controller), START | A second skater spawns 1.5 m to the first player's right; the window splits left (player 1) and right (player 2). |
| Pause | either pad, START | The pause menu opens and only the pad that opened it drives it until it closes. If both press START on the same frame, player 1 owns it. |
| Leave | pad 1, hold BACK about 2 s (120 frames) | The second skater despawns and the full-window view returns. |
| Rejoin | pad 1, START | As Join; the co-op scoreboard starts again. |

Pads are read through the platform pad layer (SDL3 on macOS and Linux, so DualSense, DualShock 4
and Xbox pads work; XInput on Windows). Player 1 is pad 0 and keeps every reference behaviour:
career, challenges, menus and the HUD are theirs.

The second player plays the first player's current character (a marquee skater or the created
skater). There is no character select for them.

## What both players get

- **Their own skater**, spawned as a Party Play member (`try_spawn_party_skater`) on player
  index 1, with a `PlayerUI` focus and pad sample in the same command queue as player 1's.
- **Their own half of the window.** The right half is player 1's camera rig carried to player
  2: the first camera's offset from player 1's hips, turned to player 2's travel heading,
  placed around player 2's hips. Both halves share an eased lens shift so each skater sits
  where the full view would frame them.
- **Labels.** Each half names the other player over their skater, or at the half's edge
  toward them, with the distance between the players.
- **Scores** (Phase 8). Each player's trick lines are banked as the trick HUD banks them (a
  completed sequence that did not end in a wipeout or interruption). Each half's outer top
  corner shows that player's co-op score, and the team total sits over the divider.
- **A regroup tether.** World collision streams around player 1 only
  (`docs/evidence/stream-focus.md`, 195 m). A co-op skater more than 120 m from player 1 on
  the ground plane is moved beside them, then the tether waits 30 ticks so the published
  root can catch up.
- **A split-screen HUD** (Phase 7). Player 1's HUD elements and chase arrows are scaled into
  the left half, keeping their shape and their top, bottom or middle anchoring. Menus,
  including the pause menu, stay full-window at their original position.

## PC-only changes to reference paths

These run only while a co-op player is joined:

- `SkaterManager::reserve_human_slot` (`ai/skater_manager.rs`), called through
  `try_reserve_coop_slot` (`session/selected_session_ai.rs`): the ambient AI skater in
  player slot 1 is unspawned and the slot is held. Without this, `SpawnAi` replaces the
  co-op player's backend row (`EntityReplacementPending`).
- `regroup_transform`: the join spawn and the tether place player 2 beside player 1's
  current root. Before that root is published, player 2 spawns at the session's spawn moved
  1.5 m along X.
- `TwoPlayerSession::route_frontend_input`: the pause-owner routing.
- The renderer's split passes (`gpu_renderer/split_screen.rs`) are the stereo renderer's
  per-eye passes, used as halves.

## Automated verification

`two_player_demo.rs` scripts two raw pads from the native main menu:

- Pad 0 enters gameplay.
- Pad 1 joins, pushes, steers and ollies.
- Pad 1 opens the pause menu. Pad 0's D-pad must not move it, pad 1's D-pad must, and pad 1's
  START closes it.
- Pad 1 leaves by holding BACK and joins again.
- Pad 0 pushes and ollies throughout.

The run fails and lists each step that did not happen. These checks must pass:

- Player 2 spawns, and each pad's pushes move its own skater.
- Frames are drawn split-screen.
- Pause ownership follows the pad that opened the menu.
- Leaving and rejoining both happen.
- Each player's first banked line comes after their own ollie, and player 1's comes before
  player 2's ollie, so each line lands on its own player's scoreboard.

```sh
mkdir -p /tmp/coop/save && cp /path/to/a/save/SKATER.P /tmp/coop/save/   # a scratch copy
SKATE2_GAME_ROOT=/path/to/skate2 SKATE2_SAVE_ROOT=/tmp/coop/save \
  ./target/release/skate2-pc --live-skating --live-controls-demo two-player-split-screen
```

Last release run (macOS, arm64), exit status 0: each ollie banked on its own player's
scoreboard. Player 1 banked 2 at gameplay tick 236 and player 2 banked 10 at tick 338, for a
team total of 12 over 987 gameplay ticks. Player 1 skated 57.4 m and player 2 60.9 m, over
2041 split-screen frames. Repeated runs bank the same lines at the same ticks; the distances
and split-frame counts vary slightly with frame timing. Screenshots of an earlier run show both
skaters framed in their halves, the labels, the P1/P2/TEAM scores and the full-window pause
menu.

Unit tests cover:

- The router's join and leave hold (`input/two_player_input_router.rs`).
- Session state, pause routing, scoring and `regroup_transform` (`session/two_player_session.rs`).
- The AI slot hold (`ai/skater_manager.rs`).
- The demo's verifier (`two_player_demo.rs`).

## Limitations

- **Career and challenges are player 1's.** Phase 8's co-op scoring is a PC team
  scoreboard, not the career's. `TwoPlayerObjective` models shared objective rules (one pass
  passes both, both must fail to fail) but is not connected to the challenge runtime. AI
  challenges get no AI skater in slot 1 while player 2 is joined.
- **Online and Party Play are single local player** (Phase 9). Online games hold one local
  skater per console, and Party Play's later players take turns on the same player indices,
  so the co-op skater does not spawn in either. A joined player 2 stays joined and spawns
  again when a free-skate or career game loads. Online co-op is not implemented.
- **Rendering follows player 1.** The shadow cascades and crowd targets are drawn for player
  1's camera, and the right half samples them. PostFX and the final pass run once over the
  whole window. The 3D world panel (`AptRenderingIntegration::Render3D`) draws only in player
  1's half. Deck ambient occlusion and online name tags are off while the screen is split. Split screen is off in replays and with
  stereoscopy.
- **Player 2's camera has no collision.** It copies player 1's camera offset, so walls can
  come between player 2's camera and their skater.
- **No character select for player 2**, and the pause menu has no "leave co-op" entry
  (leave with BACK).
- **Real-controller testing** is by hand. The automated demo drives raw pad samples through
  the same pad layer, not physical controllers.

## Files

| File | Role |
| --- | --- |
| `crates/skate2-core/src/input/two_player_input_router.rs` | Pad 1's START join and BACK-hold leave. |
| `crates/skate2-core/src/session/two_player_session.rs` | Co-op state, pause owner, scoreboard, tether constants, `regroup_transform`. |
| `crates/skate2-core/src/session/selected_session_freeskate.rs` | Spawn, removal, scoring feed, tether (`try_spawn_player2`, `try_remove_player2`, `update_coop`). |
| `crates/skate2-core/src/session/selected_production_input_owner.rs` | Player 2's `PlayerUI` focus and sample, and pause-menu input. |
| `crates/skate2-core/src/ai/skater_manager.rs`, `session/selected_session_ai.rs` | The AI slot hold. |
| `apps/skate2-pc/src/live_skating_client.rs` | Calls the router every tick. |
| `apps/skate2-pc/src/coop_camera.rs` | Player 2's camera and the shared lens shift. |
| `apps/skate2-pc/src/gpu_renderer/split_screen.rs` | The two half passes. |
| `apps/skate2-pc/src/marker_ui/coop_overlay.rs` | Labels and scores. |
| `apps/skate2-pc/src/marker_ui.rs` (`fit_split_hud`) | The HUD fitted into the left half. |
| `apps/skate2-pc/src/two_player_demo.rs` | The automated demo and its verifier. |
