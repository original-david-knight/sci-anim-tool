# Render a video

`sci_anim render <scene file> -o <mp4>` renders the whole scene to an H.264 MP4 video with ffmpeg, with the scene file's sounds mixed under it. The report gives `mp4`, `width`, `height`, `fps`, `frames`, `duration` and `gpu`, the GPU it drew on (see `gpu.md`). With `--updates`, stderr gets one JSON render update a line: `drawing` about once a second, then `finishing`, then `finished`.

## Sub-features

- `render-mp4` writes a video with `frames` = `round(duration * fps)` frames at the scene config's size, each side rounded up to an even number.
- `render-sounds` mixes each sound from the start of its chapter, so the video has an AAC stream.
- `render-updates` prints render updates on stderr and leaves the report on stdout unchanged.
- `render-transparent-refused` refuses a transparent scene with a `usage` error, exit 2.
- `render-io` refuses a path whose parent folder is missing, with an `io` error, exit 2.

## How to get to it (user POV)

- Run `sci_anim render <scene file> -o <mp4>`, with `--updates` for a long render.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. Doctor's ffmpeg and GPU lines are `ok`.

- **Video.** Run `$SKILL/scripts/run "$RUN" render-mp4 render examples/timeline.json -o "$RUN/timeline.mp4" --updates`. Exit 0, `.frames` 180, `.duration` 6.0. `ffprobe -v error -show_entries stream=codec_name,width,height,nb_frames -of compact "$RUN/timeline.mp4" | tee "$RUN/timeline.ffprobe"` prints `codec_name=h264|width=1280|height=720|nb_frames=180`.
- **Updates.** `tail -2 "$RUN/render-mp4.stderr"` shows `{"update":"finishing",...}` then `{"update":"finished","frames_drawn":180,"frames":180}`. Every stderr line parses as JSON: `jq -c . "$RUN/render-mp4.stderr"` exits 0.
- **Sounds.** Run `$SKILL/scripts/run "$RUN" render-sounds render examples/narrated_swap.json -o "$RUN/narrated.mp4"`. Exit 0, `.frames` 333. `ffprobe -v error -show_entries stream=codec_type,codec_name -of compact "$RUN/narrated.mp4"` lists a `video` `h264` stream and an `audio` `aac` stream.
- **Transparent refused.** Run `$SKILL/scripts/run "$RUN" render-transparent-refused render examples/transparent_overlay.json -o "$RUN/work/x.mp4"`. Exit 2, `.errors[0].kind` is `usage`, the message points to `sci_anim frames`, and `$RUN/work/x.mp4` does not exist.
- **Missing folder.** Run `$SKILL/scripts/run "$RUN" render-io render examples/timeline.json -o "$RUN/nodir/x.mp4"`. Exit 2, `.errors[0].kind` is `io` with `.errors[0].file` naming the path, and `$RUN/nodir` does not exist.
- **Proof.** The MP4s, their `ffprobe` lines and the reports in `$RUN`. For a visual claim, also take `ffmpeg -v error -ss <seconds> -i <mp4> -frames:v 1 "$RUN/<name>.png"` at the scene times around the change and view them.

## Gotchas

- `narrated_swap.json` is 1920x1080 and 11.1 s. It takes several seconds. Start it in the background with its PID in `$RUN/pids` when you need to do other work meanwhile, as the SKILL's Cleanup shows.
- `render` overwrites an existing MP4 at the path. Use a fresh name per run.
- The report alone says whether the render worked. A `finished` update without an ok report is not success.
