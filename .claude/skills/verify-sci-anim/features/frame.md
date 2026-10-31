# Render one frame

`sci_anim frame <scene file> --scene-time <seconds> -o <png>` draws the frame at one scene time to a PNG, so an agent can look at a moment of the video without rendering all of it. The report gives `png`, `width`, `height`, `scene_time` and `gpu`, the GPU it drew on (see `gpu.md`).

## Sub-features

- `frame-opaque` draws the frame on the scene config's background, `#1e1e1e` by default.
- `frame-transparent` keeps pixel alpha when `scene_config.background` is `transparent`.
- `frame-mid-animation` shows an animation part way through its span.
- `frame-out-of-range` refuses a scene time outside the scene, exit 1.

## How to get to it (user POV)

- Run `sci_anim frame <scene file> --scene-time <seconds> -o <png>`.
- `-t` is the short form of `--scene-time`, and `--t`, the alias older skills taught, still works. A negative scene time is parsed, then refused as out of range.
- `check-layout` draws the frame at each step time of its contact sheet, as `frame` does.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. Doctor's GPU line is `ok`.

- **Opaque frame.** Draw the timeline example at 1 s. Run `$SKILL/scripts/run "$RUN" frame-opaque frame examples/timeline.json --scene-time 1 -o "$RUN/timeline-1s.png"`. Exit 0, `.width` 1280, `.height` 720. View the PNG. It shows a cyan circle with a dark fill on the left and a red lit sphere right of center, on dark grey.
- **Mid-animation.** The orb's move runs from 1 s to 2.5 s. Draw it at 1, 1.75 and 2.5: `$SKILL/scripts/run "$RUN" frame-mid-animation-1.75 frame examples/timeline.json -t 1.75 -o "$RUN/timeline-1.75s.png"`, and the same with `1` and `2.5`. Run `inspect` at each. The orb sits where `inspect` puts it: at `[1, -1, 0]`, then about `[3.18, 1.18, 0]`, then at `[3, 1, 0]`. Its `overshoot` easing curve carries it past its end at 1.75.
- **Transparent frame.** Run `$SKILL/scripts/run "$RUN" frame-transparent frame examples/transparent_overlay.json --scene-time 1 -o "$RUN/overlay-1s.png"`. Exit 0, 640x360. `file "$RUN/overlay-1s.png"` says `8-bit/color RGBA`. The top-left pixel is empty: `ffmpeg -v error -i "$RUN/overlay-1s.png" -vf crop=1:1:0:0 -f rawvideo -pix_fmt rgba - | od -An -tu1` prints four numbers ending in `0`.
- **Out of range.** Run `$SKILL/scripts/run "$RUN" frame-out-of-range frame examples/timeline.json -t 99 -o "$RUN/work/late.png"`. Exit 1, code `scene_time_out_of_range`, and no PNG is written.
- **Proof.** The PNGs and reports in `$RUN`, and a sentence per PNG saying what it shows.

## Gotchas

- A transparent PNG viewed as is shows on white, so white text vanishes. Composite it first, as `frames.md` shows.
- `examples/sphere_and_circle.json` has no timeline. Its only scene time is 0.
- The last valid scene time is the `duration` from `validate`. Pass that, not `duration + 1/fps`.
- The PNG is overwritten without a warning. Give each frame its own name.
