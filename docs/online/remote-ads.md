# Remote billboard ads (PC only)

The original showed ads on the world's billboards that it fetched from a web server
(`Sk8::Net::Massive`, Massive Ad Client 3; [services.md](services.md) §5). The offline reference
build has no such client in the port, so every `advertisement` billboard showed the placeholder
texture its model was authored with.

This page describes the port's stand-in. It is a **PC-only addition**, not a reconstruction:

| Part | Confidence |
| --- | --- |
| Which materials show ads (`advertisement` effect) | `medium`: the effect has no recovered shader source ([GRAPHICS.md](../GRAPHICS.md)); that the original swapped these materials' diffuse for a fetched ad is inferred from the effect's name, the `Massive_Ad_*` material names and the `Massive` client |
| The protocol below | `hypothesis`: the port's own, nothing of the original's wire format is recovered |
| Placeholder-to-ad matching (size, then shape, then GUID) | `hypothesis`: the original's placement rules are not recovered |
| Timed rotation and PNG-to-JPEG conversion | `hypothesis`: PC-only policies, not recovered Massive behaviour |

Nothing here runs unless the game is built with the `remote-ads` cargo feature, which the stock
build leaves off, so the stock build's behaviour is unchanged.

## Parts

| Part | Where |
| --- | --- |
| PNG directory / test ad server (Go, standard library only) | `tools/ad-server/` |
| Discovery, manifest, HTTP client, ad selection, mip chain | `crates/skate2-net/src/ads.rs` |
| JPEG decode, GPU upload cache, timed rotation, the `remote-ads` gate | `apps/skate2-pc/src/remote_ads.rs` |
| The swap | `TextureResolver::remote_ad` and `environment_material` in `apps/skate2-pc/src/render_inspection.rs` |

## Protocol (`skate2-ads 1`)

Plain HTTP/1.0 `GET`s, all under `/skate2-ads/v1`:

| Request | Answer |
| --- | --- |
| `/ping` | `skate2-ads 1` |
| `/manifest.txt` | `skate2-ads 1`, then one `ad <id> <width> <height> <path>` line per ad |
| `<path>` from the manifest | the ad as a baseline or progressive RGB JPEG of exactly `<width>` x `<height>` |

The client accepts a manifest only when its first line is the header, skips malformed lines (an
id of anything but letters, digits, `-`, `_`, `.`; a size outside 1..=2048; a path that is not
an absolute, space-free request path), keeps at most 64 ads, and limits answers to 64 KB for the
manifest and 4 MB for an image. An ad whose JPEG is not RGB or not the listed size is skipped.

## Discovery

The game looks once, the first time the map is built, and never again in the session:

1. `SKATE2_ADS_URL` (`host`, `host:port` or `http://host:port`), if set. Only this server is tried.
2. Otherwise `127.0.0.1:8745`, then `[::1]:8745` (the port the stock server listens on).

A server counts when `ping` answers `skate2-ads 1` and the manifest lists at least one ad that
decodes. Otherwise the game does nothing and keeps the world's own billboards, as the stock build
does. A failure to connect costs at most 250 ms; a loopback port with no server refuses at once.

## Which ad goes where

For each `advertisement` material, the diffuse channel's placeholder texture is looked up through
the shader texture table (the same path every material texture takes), and its size is read from
the original texture:

1. Among the ads of that exact size, else of that aspect ratio, pick `ads[slot % count]`.
2. `slot` is the high 32 bits of the placeholder's GUID, which is its asset number: every billboard
   of one placeholder agrees, and the numbered variants of a sign (`..._v_01`, `_02`, `_03`) get
   different ads.
3. With no ad of that shape, or for a placeholder that is a PROXY texture, the billboard keeps its
   own texture.

Only the diffuse channel is replaced; the lightmap, chromaticity, shader and draw order are the
`advertisement` effect's own. The ad is uploaded as an A8R8G8B8 texture with a box-filtered mip
chain, and bind groups are cached by the initially selected ad's index in the manifest.

## PNG files and rotation

The Go server accepts `-ads <folder>` to read top-level `.png` files in lexicographic filename
order. It converts them to RGB JPEG (quality 95), compositing alpha over white. RGB/RGBA,
grayscale and indexed PNGs are accepted, up to 2048 pixels per side and 16 MiB per source file.
At most 64 images are allowed, and each converted JPEG must fit the client's 4 MiB limit.
Invalid input is a startup error, rather than a silently missing ad.

For the packaged example, use `dist/skate2-pc-withads/ads/`. Suggested dimensions:
256x128 / 512x256 horizontal, 128x256 vertical, 128x128 square, 64x64 / 64x32 small signs.
`-write-examples <folder>` writes four numbered, brightly coloured PNGs of each size without
overwriting existing files. See the [server guide](../../tools/ad-server/README.md).

Every 10 wall-clock seconds the client advances through manifest entries of the **same exact
dimensions as the initially selected ad**, starting at that ad's position and wrapping.
`SKATE2_ADS_ROTATE_SECONDS` on the game sets the interval in whole seconds; `0` disables it.
Different-size images of the same aspect ratio are candidates for the initial fallback, not
frames of one rotation. A lone image remains static.

Each rotation rewrites the existing GPU texture and all its mip levels, so retained material
bindings and streamed map generations stay valid. A scene otherwise marked unchanged is
redrawn when rotation is due. There is no network work during rotation, and no refresh of the
inventory: **restart both server and game after editing, adding or removing PNG files**.
The clock continues through pause/minimize; the next rendered frame catches up.

## Running it

```sh
# the server (any machine; stock address 127.0.0.1:8745)
cd tools/ad-server
go run . -write-examples ./ads
go run . -ads ./ads -addr 127.0.0.1:8745

# the game, built with the feature
cargo build --release -p skate2-pc --features remote-ads
```

In a diagnostic launch (`--live-skating` or `--render-inspection`, without `--quiet`), the build
logs `remote ads: N ads from <host>` when a server is found and, for every map generation,
how many `advertisement` draws took an ad. A normal no-argument launch suppresses these logs.
With `SKATE2_ADS_LOG=1` it also logs rotation steps and each billboard's world position
(the centre of its vertices), which is the way to find one in the world.
The test server's ads are bright solid colours with stripes, a border, a white triangle in the
top-left corner (so a mirrored or rotated ad is obvious) and the ad's number and size in text.

## Validation

The PNG/rotation extension was checked against the real external game corpus:

- Go tests, race tests and vet: PNG loading/order, RGB/RGBA/grayscale/indexed conversion,
  transparent pixels becoming white, malformed/oversize/excess inventory rejection, example
  generation and overwrite refusal, plus manifest/image HTTP round trips.
- 13 focused Rust ad tests: discovery, HTTP/manifest bounds, matching, mip generation and
  rotation ordering/wrap/slot offsets without crossing texture dimensions.
- Release rendering loaded 24 PNG-derived ads and replaced 43 advertisement draws. Repeated
  captures of the same waterfront kiosk showed its numbered artwork changing, including
  wrapping from AD 4 to AD 1; only GPU pixels changed, not the retained material bindings.
  The probe used a two-second interval to exercise multiple steps quickly.
- With discovery pointed at an unreachable loopback port, the same capture was byte-identical
  to the unchanged stock executable. All diagnostic runs used a copied save root.
- The feature release build passed, new Rust modules passed formatting, and app Clippy
  reported no diagnostics on changed code. The repository still has unrelated Clippy warnings;
  this is not a claim of a clean workspace-wide strict gate.

## Unknowns

- The original wire protocol, its ad inventory and its placement rules (which billboards, per
  region, per session, rotation) are not recovered.
- Impression reporting and click-through, if the original had them, are not modelled.
- The effect's gamma handling of the replaced diffuse is the placeholder's, so an ad is shaded
  exactly as a placeholder of the same size would be.
