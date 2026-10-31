# Render a frame sequence

`sci_anim frames <scene file> -o <directory>` renders every frame into a new directory as numbered PNGs, keeping pixel alpha, with no ffmpeg and no sounds. The report gives `directory`, `pattern` (`frame-%06d.png`), `start_number` 0, `width`, `height`, `fps`, `frames`, `duration`, `alpha` `straight`, `color_space` `srgb` and `gpu`, the GPU it drew on (see `gpu.md`).

## Sub-features

- `frames-count` writes `max(1, round(duration * fps))` PNGs, frame `i` at scene time `i / fps`.
- `frames-transparent` keeps straight alpha in RGBA PNGs for a transparent scene.
- `frames-marker` keeps an `.incomplete` marker in the directory until the last frame is written.
- `frames-existing` refuses a directory that exists, with an `io` error, exit 2, and leaves it unchanged.
- `frames-updates` prints render updates on stderr, as `render` does.

## How to get to it (user POV)

- Run `sci_anim frames <scene file> -o <directory>`, with `--updates` for render updates.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. Doctor's GPU line is `ok`. ffmpeg is not needed.

- **Sequence.** Run `$SKILL/scripts/run "$RUN" frames-transparent frames examples/transparent_overlay.json -o "$RUN/overlay-frames" --updates`. Exit 0. The report has `.frames` 33, `.fps` 12, `.duration` 2.75, `.alpha` `straight`.
- **Files match the report.** `ls "$RUN/overlay-frames" | tee "$RUN/overlay-frames.ls" | wc -l` prints `33`, from `frame-000000.png` to `frame-000032.png`. `ls -A "$RUN/overlay-frames" | grep -c incomplete` prints `0`.
- **Transparency.** `file "$RUN/overlay-frames/frame-000012.png" | tee "$RUN/overlay-frames.file"` says `640 x 360, 8-bit/color RGBA`. Frame 0 (scene time 0, before the fades) is empty: `ffmpeg -v error -i "$RUN/overlay-frames/frame-000000.png" -vf alphaextract,format=gray -f rawvideo - | od -An -tu1 -v | tr -s ' ' '\n' | sort -u` prints only `0`. Composite frame 12 (1 s) on dark grey and view it: `ffmpeg -v error -f lavfi -i color=c=0x1e1e1e:s=640x360 -i "$RUN/overlay-frames/frame-000012.png" -filter_complex overlay -frames:v 1 "$RUN/frame-000012-on-dark.png"`. It shows the white heading `Constant speed`, a cyan disc and the yellow math `x = vt`.
- **Existing directory.** Run the same command again into `$RUN/overlay-frames`. Use the check name `frames-existing`. Exit 2, `.errors[0].kind` `io`, and the message contains `File exists`. `ls "$RUN/overlay-frames" | wc -l` still prints `33`.
- **Missing parent.** Run with `-o "$RUN/work/none/seq"`, as check `frames-no-parent`. Exit 2, `io`, and `$RUN/work/none` still does not exist.
- **Proof.** The directory, its listing, the reports and the two viewed PNGs in `$RUN`.

## Gotchas

- An image viewer shows a transparent PNG on white, so white text disappears and a transparent frame looks broken. Composite it on a dark color before you view it.
- The destination must not exist and its parent must. `new-run` makes `$RUN`, so use a new name inside it.
- A frame sequence has no sounds even when the scene file lists them. Sound files are still checked by validation.
- An opaque scene renders here too, as RGBA with pixel alpha 1. That does not prove transparency. Use a scene whose `background` is `transparent`.
