# Narration and sound

Read `instructional-videos.md` and `motion.md` first.

## The voice explains, the screen shows

When the video has a voice, the voice explains and the screen shows. Everything in "Text on screen" and "Pacing" of `instructional-videos.md` about reading time is for silent videos. These rules replace it.

- Show only short labels: a name, a value, a term, an equation, at most about four words. Never put a sentence on screen that says what the voice says. The viewer cannot read and listen at once, and the video's subtitles already show the words, so a paraphrase on screen clashes with both.
- Keep the frame clear where the player shows subtitles. Pass that box to `sci_anim check-layout` as a reserved region, in frame pixels from the top left: `--reserve 0,860,1920,1080` keeps about the bottom fifth of a 1080p frame clear. It reports every shown object in the region, with its object id and the scene times it is there, at the scene times it checks and not every frame between them. Each contact sheet tile shows the region as a magenta box. The full-size frames show the frame as the video does. For a box that is covered for part of the video only, such as a photo laid over one chapter, put its span of scene time after the box, with sci_anim 0.6.0 or later: `--reserve 1200,100,1800,600@12-30` keeps it clear from scene time 12 to 30 seconds, and the contact sheet outlines it only on the tiles in that span. A span with END before START, or outside the scene, is a bad command line and exits 2. Pass `--reserve-exempt <object id>` for an object that may sit there, and pass `--reserve` once for each region. Drop the sentence place at the bottom. A heading of a few words at the top is still fine.
- Let the narration set the pace. Make one `chapter` per voice clip, with the clip's length as its `min_duration`, and put the clip in `sounds` at that chapter. The chapter then lasts at least as long as its clip, and adding a step to one chapter moves no other clip out of place. Measure a clip with `ffprobe -v error -show_entries format=duration -of csv=p=0 pivot.mp3`, and add about 0.4 s so one clip does not run into the next.
- Time each change to the word that names it. A text-to-speech service such as ElevenLabs returns each word's start in seconds from the start of the clip. In a chapter whose first step is a `play`, a word at 1.42 s is `"start": 1.42` on the animation in that step. In a later step of the chapter, subtract that step's start in the chapter. Start a change on its word, or up to 0.1 s before it: the eye should not lag the ear.
- Give each change at least 0.4 s. A change the voice does not mention needs no word to land on, but it still needs time to be seen.

A narrated chapter, where the 6 turns blue as the narrator says "six":

```json
{ "type": "chapter", "id": "pivot", "min_duration": 4.8, "steps": [
  { "type": "play", "duration": 4.8, "animations": [
    { "type": "recolor", "object": "box6", "stroke": "#58c4dd", "fill": "#1d4a5a", "start": 1.42, "end": 1.9 },
    { "type": "move", "object": "box6", "to": [-3, 1, 0], "start": 2.6, "end": 3.4 }
  ] }
] }
```

## Sound files

List each sound file in the scene file's `sounds`, with the chapter it starts at:

```json
"sounds": [{ "file": "voice/pivot.mp3", "at": "pivot" }, { "file": "voice/swap.mp3", "at": "swap" }]
```

`render` mixes every sound under the video at the loudness it has and cuts it at the video's end. A path is relative to the scene file, and the file must be on disk, because the tool never downloads. `validate` reports each sound's `start` in seconds. Turn music down well under a voice, such as `-af volume=0.15`, and start it at a chapter that begins at scene time 0.

Level the voice clips before the render, because the tool does not change their loudness. Level each clip in two passes, to -16 LUFS with true peaks under -1.5 dBTP. The first pass measures the clip and prints its numbers as JSON:

```sh
ffmpeg -i raw.mp3 -af loudnorm=I=-16:TP=-1.5:LRA=11:print_format=json -f null -
```

The second pass gives those numbers back, `input_i` as `measured_I`, `input_tp` as `measured_TP`, `input_lra` as `measured_LRA`, `input_thresh` as `measured_thresh` and `target_offset` as `offset`, and turns the whole clip up or down by one gain:

```sh
ffmpeg -i raw.mp3 -af loudnorm=I=-16:TP=-1.5:LRA=11:measured_I=-20.21:measured_TP=-2.22:measured_LRA=0.00:measured_thresh=-30.43:offset=3.48:linear=true -ar 48000 pivot.mp3
```

`loudnorm` writes 192 kHz, so `-ar 48000` sets the rate back. Then measure the result rather than assume it: `ffmpeg -i pivot.mp3 -af ebur128=peak=true -f null -` prints its integrated loudness `I` and its true peak in its summary. Where the gain would push a peak past -1.5 dBTP, the clip ends quieter than -16 LUFS. If the clips end more than 1 LU apart, level them all again to the quietest one's `I`, so no voice jumps in loudness between chapters.

Check where each sound starts in the scene, not in the waveform. `validate` reports each sound's `start`, and `render` places the sound exactly there. Check that each `start` is the chapter the sound belongs to.

Silence detection is a separate check, of the voice clips themselves. `ffmpeg -i out.mp4 -af silencedetect=noise=-40dB:d=0.2 -f null -` prints a `silence_end` where sound begins after silence. It shows where a voice clip starts only when nothing else plays under it. Under a music bed there is no silence to find. To see where a clip starts in the waveform, render without the music, by taking it out of `sounds`, or trust `validate`.
