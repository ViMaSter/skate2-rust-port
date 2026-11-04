# skate2-ad-server

A local stand-in for the web server the original game fetched its in-game billboard ads from.
It serves generated test ads in glaring colours (magenta, lime, cyan, orange stripes with a
border, a white top-left triangle and the ad's number and size), one set per billboard texture
size the world uses, so a billboard that shows one is obvious.

```sh
go run .                              # generated ads on 127.0.0.1:8745
go run . -write-examples ./ads         # writes 24 PNGs, then exits; never overwrites
go run . -ads ./ads                    # serves your PNG files instead
go run . -ads ./ads -addr 0.0.0.0:8745 # reachable from another machine
go test ./...
```

The game finds it by itself on `127.0.0.1:8745` when built with the `remote-ads` feature
(`SKATE2_ADS_URL` names another server). The protocol is the port's own and the whole feature a
PC-only addition: see [docs/online/remote-ads.md](../../docs/online/remote-ads.md).

## Your PNG ads

For the packaged example, put images directly in **`dist/skate2-pc-withads/ads/`** and run
`./skate2-ad-server -ads ./ads` from `dist/skate2-pc-withads/`. With `go run`, `-ads` is
relative to your current working directory. Subfolders are not scanned.

- Use static `.png` files (case-insensitive extension). RGB, RGBA, grayscale and indexed PNGs
  work; 8-bit RGB is recommended. Transparency is composited onto **white**.
- Recommended sizes: **256x128** or **512x256** for horizontal billboards, **128x256** for
  vertical billboards, **128x128** for squares; small signs also use **64x64** and **64x32**.
- Other sizes with the same aspect ratio work if no exact-size ad exists. No stretching,
  cropping or resizing is performed. Maximum 2048 pixels per side, 16 MiB per PNG, 64 ads.
  The server converts images to RGB JPEG at quality 95; that result must fit within 4 MiB.
- Files are sorted lexicographically by filename. Prefix names with `01-`, `02-`, etc.
  No hand-written manifest is needed. Invalid files cause an explicit startup error.
- **Restart both server and game after changing files.** Inventory is read at server startup
  and fetched once when the game first loads its map; there is no hot reload.

## Example rotation

`-write-examples` creates four numbered images of each recommended size:
magenta **AD 1** -> lime **AD 2** -> cyan **AD 3** -> orange **AD 4** -> repeat.
The ads-enabled game advances every **10 seconds**, reusing the existing GPU bindings.
Each billboard starts at its GUID-derived offset; numbered variants can show different ads
at the same time. Rotation includes only files with the **same pixel dimensions** as its
initially selected ad. Use a consistent size within each intended rotation.

Set `SKATE2_ADS_ROTATE_SECONDS=5` on the **game** for a five-second interval, or `0` to disable
rotation. Rotation uses wall time, including pauses; all images are downloaded up front, so
no network requests are needed during rotation. `SKATE2_ADS_LOG=1` logs each rotation.
