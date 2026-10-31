# Overlays: frames to lay over other footage

To place objects over external images or video, set `scene_config.background` to `transparent` and render with `sci_anim frames s.json -o frames`. `render` refuses a transparent scene.

- The PNGs have straight alpha and sRGB colors. Composite in linear light.
- The report gives the directory, the relative filename pattern, the starting number, the dimensions, the frame rate, the frame count, the duration, the alpha representation and the color space.
- The frame sequence keeps empty frames and chapter holds, has no sounds, and uses the same scene times as an MP4 render.
- The directory must not exist, and other tools must leave it unchanged while it renders. An `.incomplete` marker stays until every frame is written. Use the sequence only after the final successful report.
- Background media, tracking, sounds and the final compositing belong to the calling tool.

The layout check cannot see the footage. It reports a text's contrast as unverified unless it can prove that an opaque rectangular panel covers the text and lies behind it. A fading or see-through panel does not hide the footage, and some shapes and camera angles stay unverified too. A text that a fade or a follow of its opacity leaves faded over a proved panel is judged at its faded opacity from the end of that span, and is not judged while its opacity changes. These notes appear in `stages.txt` and on the console even when the check exits 0, and the contact sheets cannot settle them. Review every unverified text over the real footage after compositing, before calling the video readable.
