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
| Join | pad 1 (the second connected controller), START | A second skater spawns 1.5 m to the first player's right; the window splits left (player 1) and right (player 2). Pressed while the game is still loading or travelling to its start, the spawn waits for player 1's landing and is placed beside them there. |
| Pause | either pad, START | That player's own pause menu opens over their half and only their pad drives it. Their skater brakes and the other player skates on. The game stops only while both menus are open. |
| Teleport | either pause menu, Map | That player's Challenge Map opens over their half. Picking a location or a challenge teleports only them; the other player skates on. |
| Join a teleport | the other pad, LB + D-pad right | While their half shows the prompt: they teleport to where the first player is. |
| Boot Player 2 | either pause menu | Player 1's menu lists Boot Player 2 above Quit Game. Player 2's lists it last (their own way out). Both ask to confirm, No selected first. |
| Leave | pad 1, hold BACK about 2 s (120 frames) | As Boot Player 2: the second skater despawns and the full-window view returns. |
| Rejoin | pad 1, START | As Join; the co-op scoreboard starts again. |

Player 1 cannot leave the game, only boot player 2.

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
- **Their own world around them.** The world streams around both skaters (a second stream
  focus, `docs/evidence/stream-focus.md`), so the players can skate apart. There is no
  tether: only a join spawn and joining a teleport put one skater beside the other.
- **A split-screen HUD** (Phase 7). Player 1's HUD elements and chase arrows are scaled into
  the left half, keeping their shape and their top, bottom or middle anchoring. Each
  player's pause menu and the Challenge Map are the original screens shrunk to half-width
  and centred in their player's half. Screens that fill the window (Game Settings, the Trick
  Guide, the Blog and the like) stay full-window and stop the game.
- **Shared challenge objectives** (Phase 8, `challenge/coop.rs`). In a career challenge the
  two skaters are one team with one progression. See "Shared objectives" below.
- **Online free skate together** (Phase 9, `session/online/coop.rs`). Player 2 skates beside
  player 1 in an online free skate, and the other consoles draw them. See "Online" below.

## Pause, boot and teleport

Each player has their own pause menu, and what a pause does depends on who is paused. All of it
is PC-only and has `hypothesis` confidence, except that the brake follows a reference mechanism
(below).

- **Two menus.** START opens that player's own hub over their half; only their pad drives it, and
  the other pad's buttons do not reach it. Player 1's is the original hub. Player 2's has Map,
  Game Settings and Boot Player 2 (the Quit, Profile, Blog, Replay, Edit Skater and Trick
  Guide rows stay player 1's). Up from the first row wraps to the last.
- **Boot Player 2 from either menu.** While player 2 is in, player 1's hub lists Boot Player 2
  above Quit Game and asks "Boot Player 2 from the game?". Player 2's own last row asks "Leave
  the game? Player 2 is removed from the session." Both open on No; Down selects Yes and A
  confirms, which closes the menu and removes player 2 as the BACK hold does. Player 1 has no
  row that removes them.
- **One pause brakes, two pauses stop.** `CoopPause` (`session/two_player_session.rs`) is
  `Off` with no second player, `Single` while exactly one hub is open (including that player's
  half-screen Challenge Map) and `Full` while both are open or a screen that fills the window
  is. Under `Single` no front-end state is pushed and the simulation keeps running; the paused
  player's skater loses its `PlayerUI` focus and stands braking, as a skater without the focus
  does (`PlayerUI::Update` 0x828af9f8 gives it an empty input, `PlayerInputData::Unpack`
  0x82a8c330 reads Brake held; confidence `medium`). Under `Full` the front end is pushed and
  the simulation stops. Game Settings, the audio menu, the skate profile, the trick guide, the
  replay and skater editors, the main menu, movies, the online hub, a map travel that is about to
  load, the jacked overlay and the career overlay all fill the window and stop the game for both.
- **Challenge Map teleports.** Either player opens the Map from their hub. It is the original
  screen shrunk into its owner's half. Choosing a location or challenge teleports only that
  player; the other keeps skating. Player 1 teleports through the ordinary path (the fade, the
  load and the gate on the streamed world). Player 2's skater is set to the destination
  (`Actor::SetTransform` 0x825ed698, as the AI's teleport does, `medium`) once the destination
  has streamed in. It brakes until then, and the teleport is dropped with a report if the
  destination is not resident after 30 s. The stream focus stays on the destination until the
  skater's published root is within 5 m of it, or for 2 s. Player 2's picks are plain
  teleports: they never start a challenge. A pick that needs another world (a DLC world) is
  dropped for player 2 and refused with a report for player 1.
- **Joining a teleport.** After a teleport, the other player's half shows `LB + RIGHT: join
  P{n}'s teleport` while the offer stands: one minute at most, and not at all if they are within
  25 m of the destination. The offer also lapses once the players stand within 25 m of each
  other, checked after its first 2 s because the teleporter's published root follows their
  teleport a few ticks late. LB held with D-pad right on that player's pad takes it, provided
  their own pause menu is closed and they are not travelling. The chord is consumed only when
  an offer stands, so player 1's prop HUD keeps its D-pad right otherwise. Joining as player 2
  sets them beside player 1 at once (1.5 m to the right, 0.5 m up, the join spawn's
  placement). Joining as player 1 makes the same fade, load and gate as their own teleport, to
  player 2's position.
- **The world streams around both.** The terrain keeps the simulation focus on player 1 and adds
  player 2's position, and their teleport's destination while it travels, as further stream foci
  (`docs/evidence/stream-focus.md`). Residency is doubled while the players are apart.

## Shared objectives

The reference challenge runtime has one objective group per activity and player. With a
co-op team set (`CoopObjectives::set_teams`), the career's mission frames list player 2's
skater beside player 1's (`selected_session_career.rs`). These rules are PC-only and have
`hypothesis` confidence: the reference build has no local co-op challenge to compare against.

- **One progression.** Player 1's group is the team's. It owns the HUD, the notifications,
  the challenge's completion and the save. Player 2 gets a partner group: a clone of it for
  player 2 with fresh instance ids. The partner group never joins the script's group list.
- **Either skater advances it.** While player 2's checks run, the partner group stands in the
  team group's slot. Continuous checks (speed, location, held states) use player 2's state and
  skater frame. Events carrying player 2's `PlayerID` (tricks, sequences, wipeouts) are handled
  there too. Objective states, points, completed unique tricks and notification windows are
  copied both ways around each swap. Each player keeps their own sequence, trigger-volume and
  Lua-variable history, so no event is rewritten or delivered twice.
- **Failing needs both.** An objective failure stands only while both players' latest
  evaluations fail it. A global timer still expires once; one skater's bail does not fail
  the team.
- Not in online rounds: their frames are the round members' facts, one skater per console.

`a_coop_team_shares_the_fliptrick_tutorial` (in `challenge/tests.rs`, ignored, needs
`SKATE2_GAME_ROOT`) plays `m_1a_03_tutorial_fliptricks` from the shipped Lua and Vault data.
The two players alternate the five tricks. Each one completes the team's next objective, and
the challenge completes and autosaves. With the team removed, the test fails at player 2's
first trick. Two unit tests cover the failure rule. Nobody has played a challenge together
by hand yet.

## Online

Player 2 can stay joined in an online free skate (protocol 16, PC addition):

- The console sends player 2's look (`CompanionRecipe`, at most one every 2.5 s). Player 2's
  frames (`CompanionFrames`) are the console's own frames without camera, audio or HUD. Every
  tick sends a header, and a pose at each run's end. Runs end every `INTERVAL` ticks, at a
  teleport and on the tick after it.
- The server relays the frames as `RemoteCompanionFrames` with a separate rate limit, sends
  the recipe to members who join later, and an empty recipe removes the companion.
- Other consoles draw the companion as a remote skater with the owner's name plus " (P2)".
  It has no audio and no score, and its facts count for nothing.
- At an online challenge round player 2 drops out (one skater per console; the round sets
  the local index). They can join again with START in lobby skate.

Online co-op needs the game and the server built at protocol 16. Older builds refuse the
connection.

## PC-only changes to reference paths

These run only while a co-op player is joined:

- `SkaterManager::reserve_human_slot` (`ai/skater_manager.rs`), called through
  `try_reserve_coop_slot` (`session/selected_session_ai.rs`): the ambient AI skater in
  player slot 1 is unspawned and the slot is held. Without this, `SpawnAi` replaces the
  co-op player's backend row (`EntityReplacementPending`).
- `regroup_transform`: the join spawn and joining a teleport place a skater beside the other's
  current root. Before that root is published, player 2 spawns at the session's spawn moved
  1.5 m along X. A START pressed while the game is still loading or travelling to its start
  (`career_start_pending`, `career_spawn_pending`, `map_travel` in `try_spawn_player2`) waits
  for the landing: the load's placeholder spawn is thousands of metres from the world's
  start, and a skater placed there stays there.
- The per-player pause menus (`ui/pause_menu.rs`), `CoopPause` and the pause-owner routing
  (`session/two_player_session.rs`, `session/selected_session_coop.rs`): which pad drives which
  menu, whether the simulation runs, and which skater keeps the `PlayerUI` focus (the second
  pad's front-end input comes from `try_coop_pause_menu_input`).
- The terrain's second stream focus (`session/selected_session_terrain.rs`).
- The renderer's split passes (`gpu_renderer/split_screen.rs`) are the stereo renderer's
  per-eye passes, used as halves.

## Automated verification

`two_player_demo.rs` scripts two raw pads from the native main menu, on the gameplay clock
(paused ticks count, so the pause steps keep their schedule):

- Pad 0 enters gameplay. Pad 1 presses START while the game is still loading.
- Both pads push, steer and ollie. Each ollie banks on its own player's scoreboard.
- Pad 1's pause menu alone, then pad 0's: only the paused player brakes. Pad 0's D-pad must not
  move pad 1's menu, pad 1's D-pad must, and START closes it.
- Both menus open at once with a skater moving: the game stops. Closing one runs it again.
- Player 2 leaves three ways and joins again each time: Boot Player 2 on pad 0's menu, Leave on
  their own menu (each walked with D-pad down and A), and holding BACK for 2 s.
- Pad 1's Challenge Map, over player 2's half with the game running and player 1 skating on:
  the pad walks the lists by what they show (Locations, then a group, then its first location,
  then Teleport). Only player 2 teleports. Player 1's half then offers the join; LB + D-pad
  right on pad 0 puts player 1 beside player 2.
- The same from pad 0's map, with pad 1 joining player 1's teleport.

The run fails and lists each step that did not happen. These checks must pass:

- Player 2 spawns beside player 1 once the game has landed (within 12 m), and each pad's
  pushes move its own skater.
- Frames are drawn split-screen.
- Each pause opens only its own menu, leaves the other player skating, and brakes the paused
  one (below half its speed when it opened, within the check's window). Both open stop every
  skater, from a speed that makes the stop visible.
- Each of the three ways out removes player 2 and the next START brings them back.
- Each teleport moves only its owner (more than 20 m from where their map closed), the other
  player is offered the join within 300 ticks of the landing, and the chord puts the follower
  within 12 m of the owner. The game must keep running while the map is open until a pick
  starts the teleport's own load.
- Each player's first banked line comes after their own ollie, and player 1's comes before
  player 2's ollie, so each line lands on its own player's scoreboard.

```sh
mkdir -p /tmp/coop/save && cp /path/to/a/save/SKATER.P /tmp/coop/save/   # a scratch copy
SKATE2_GAME_ROOT=/path/to/skate2 SKATE2_SAVE_ROOT=/tmp/coop/save \
  ./target/release/skate2-pc --live-skating --live-controls-demo two-player-split-screen
```

Last release run (macOS, arm64), exit status 0: each ollie banked on its own player's
scoreboard. Player 1 banked 2 at gameplay tick 236 and player 2 banked 10 at tick 338, for a
team total of 12 over 987 gameplay ticks. Player 1 skated 49.7 m and player 2 60.8 m, over
2139 split-screen frames. Repeated runs bank the same lines at the same ticks; the distances
and split-frame counts vary slightly with frame timing. Screenshots of an earlier run show both
skaters framed in their halves, the labels, the P1/P2/TEAM scores and the full-window pause
menu.

Unit tests cover:

- The router's join and leave hold (`input/two_player_input_router.rs`).
- Session state, pause routing, scoring and `regroup_transform` (`session/two_player_session.rs`).
- The AI slot hold (`ai/skater_manager.rs`).
- The demo's verifier (`two_player_demo.rs`).
- Team failures (`challenge/coop.rs`).
- The protocol-16 messages' round trips (`skate2-net` `protocol.rs`), and a server test of
  the companion stream beside its owner's (`a_companion_skater_streams_beside_its_owner`).

## Limitations

- **Career progress is player 1's profile.** Shared objectives make the team's progress
  player 1's. Cash, unlocks and the save are player 1's. The co-op scoreboard is a PC team
  scoreboard, not the career's. AI challenges get no AI skater in slot 1 while player 2 is
  joined.
- **Online co-op is free skate only** (Phase 9). Player 2 drops out at an online challenge
  round. Party Play stays single local player: its later players take turns on the same
  player indices, so the co-op skater does not spawn. A joined player 2 stays joined and
  spawns again when a free-skate or career game loads.
- **Online co-op has not been played across two machines.** The server test and the
  protocol round trips ran. A two-console session did not.
- **Rendering follows player 1.** The shadow cascades and crowd targets are drawn for player
  1's camera, and the right half samples them. PostFX and the final pass run once over the
  whole window. The 3D world panel (`AptRenderingIntegration::Render3D`) draws only in player
  1's half. Deck ambient occlusion and online name tags are off while the screen is split. Split screen is off in replays and with
  stereoscopy.
- **Player 2's camera has no collision.** It copies player 1's camera offset, so walls can
  come between player 2's camera and their skater.
- **No character select for player 2.** Their pause menu has only Map, Game Settings and Boot
  Player 2 (BACK held still removes them too).
- **Teleports are plain.** Player 1's teleport fades and loads the whole window, so player 2 is
  frozen behind the fade while it runs. Player 2's pick is set down once the destination has
  streamed in and they brake until then. A pick that needs another world is dropped for
  player 2 and refused for player 1 with a report. A teleport that player 1's career
  script makes (a challenge's start, for example) raises no join offer for player 2: only a
  Challenge Map pick, and player 2's own teleport, do.
- **The second stream focus is a PC addition and covers the terrain only.** The living world's
  DMO objects, dynamic hulls, AI skaters and NIS scenes stay with player 1, so far from
  them they may not be around player 2 (hypothesis, `docs/evidence/stream-focus.md`).
- **Career overlays, NIS scenes and popups stay player 1's.** Player 2's half does not show
  them, and a screen that fills the window stops the game for both.
- **The numbers are guesses.** The 25 m join radius, the offer's one-minute life and 2 s
  grace, player 2's 30 s streaming limit, and the 5 m / 2 s settle of the stream focus are
  `hypothesis`; so is the half-screen shrink of the hub and the map.
- **Real-controller testing** is by hand, and has not been done for the pause, boot and
  teleport steps: the automated demo drives raw pad samples through the same pad layer, not
  physical controllers. Shared objectives and online co-op were not played by hand either.

## Files

| File | Role |
| --- | --- |
| `crates/skate2-core/src/input/two_player_input_router.rs` | Pad 1's START join and BACK-hold leave. |
| `crates/skate2-core/src/session/two_player_session.rs` | Co-op state, `CoopPause`, pause and screen owner, teleport offers, player 2's travel state, scoreboard, `regroup_transform`. |
| `crates/skate2-core/src/session/selected_session_freeskate.rs` | Spawn (deferred during a game start's travel), removal, scoring feed (`try_spawn_player2`, `try_remove_player2`, `update_coop`). |
| `crates/skate2-core/src/session/selected_session_coop.rs` | Player 2's teleports and travel, join offers and the LB + RIGHT chord, player 2's hub and Challenge Map, the co-op refusals. |
| `crates/skate2-core/src/session/selected_session_public_facade.rs` | Per-player pause and screen wiring, player 1's map picks, the focus and brake wiring. |
| `crates/skate2-core/src/session/selected_session_terrain.rs` | The second stream focus (player 2's position and their teleport's destination). |
| `crates/skate2-core/src/session/selected_production_input_owner.rs` | Player 2's `PlayerUI` focus and sample, and each pad's pause-menu input. |
| `crates/skate2-core/src/ui/pause_menu.rs` | The per-player hub: Boot Player 2 on player 1's, player 2's own rows. |
| `crates/skate2-core/src/ai/skater_manager.rs`, `session/selected_session_ai.rs` | The AI slot hold. |
| `apps/skate2-pc/src/live_skating_client.rs` | Calls the router every tick and hands the demo its observations. |
| `apps/skate2-pc/src/coop_camera.rs` | Player 2's camera and the shared lens shift. |
| `apps/skate2-pc/src/gpu_renderer/split_screen.rs` | The two half passes. |
| `apps/skate2-pc/src/marker_ui/coop_overlay.rs` | Labels, scores and the join prompts. |
| `apps/skate2-pc/src/marker_ui.rs` (`fit_split_hud`) | The HUD fitted into the left half, and the hub and Challenge Map fitted into their owner's half. |
| `apps/skate2-pc/src/city_streaming.rs` | The presentation and texture selection for both stream foci. |
| `apps/skate2-pc/src/two_player_demo.rs` | The automated demo and its verifier. |
| `crates/skate2-core/src/challenge/coop.rs` | Teams, partner objective groups, team failures. |
| `crates/skate2-core/src/session/online/coop.rs` | Player 2's online recipe and frames. |
| `crates/skate2-net/src/{protocol,client,server}.rs` | The protocol-16 companion messages and relay. |
