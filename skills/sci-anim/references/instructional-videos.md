# Making an instructional video

A good instructional video answers one question, shows one idea at a time, and gives the viewer time to read and think. In a silent video the text on screen carries the whole explanation. In a narrated video the voice carries it and the screen shows it: read `narration.md` before you plan one.

## Write the lesson plan first

Before any JSON, write the plan as a short list:

1. The question the video answers, in one sentence, such as "Why is the derivative of x² equal to 2x?"
2. Who watches it and what they already know.
3. Three to six ideas, in order. For each idea, write the one sentence the viewer reads, or in a narrated video the sentences the narrator says, what appears, what moves or changes, and how many seconds each of those takes by the pacing table below. A change carries the idea. Text only names it.
4. A last idea that sums up the answer on one screen.

If the user gave no length, aim for 45 to 120 seconds.

## Show, then say

- Open on the thing itself, moving or being drawn, within the first 3 seconds. A title alone on a dark frame is not an opening.
- Draw the thing before you explain it: the shape, the curve, the molecule. Then write one short sentence about it.
- Pick a thing the viewer recognizes, whose change is the idea: a balance that tips, a tank that fills, cards that swap places, a pizza cut into slices. Rows of labeled boxes read as nothing, and so does one big shape with nothing on it to discover.
- Make the change the lesson is about happen on screen. Morph `a^2 + b^2` into `c^2`, slide a secant line into a tangent, let atoms move in a reaction, lay a circle's slices out into a rectangle. A viewer remembers a change they watched far better than a sentence.
- Show the reason, not only the claim. When a sentence says why, such as "its width is half the rim", put the why on the picture: split the rim in two and label each half πr. A reason that lives only in a sentence is told, not shown.
- Move many pieces as one. When slices, atoms or cards go somewhere, start them together and send them on paths that do not cross. Pieces flying in one at a time and crossing on the way read as clutter, and doing it again for each idea costs the time the lesson needs.
- Tie the symbol to the picture. When a quantity gets a name, such as `A` or `T = 2 s`, put it on the thing it measures, in that thing's color, rather than only in a sentence.
- Build up step by step. Add one object or one term at a time, never a finished diagram.
- End each idea on a frame that would make sense as an illustration in a textbook. End the video by bringing the picture back to where it started, or by showing the answer beside the picture that proved it, never on a screen of text alone.

## Text on screen

- Write sentences of at most about 12 words. Write one sentence at a time, and fade it out before the next.
- Keep two places for text and use them for the whole video: a heading at the top (y about 3.3, `size` 0.5) and a sentence at the bottom (y about -3.4, `size` 0.4). Everything else goes between them.
- A heading is the question the video answers, or the name of the idea on screen now. It is never the name of a tool feature. Change it when the idea changes.
- Never set text smaller than `size` 0.3. At 1080p, 0.3 is about 40 pixels tall. `check-layout` lists text too small or too faint to read. A text dimmed and left faded is judged at its faded opacity: white on `#101824` reads down to about opacity 0.25, and a dimmer color needs more.
- A sans text is about 0.45 × `size` × characters wide. A 50-character sentence at `size` 0.4 is about 9 world units wide, which fits. Math has no such rule: a fraction or a long equation is far wider than its characters suggest. Read its width from `bounding_box` in `inspect` before you place things beside it.
- Write text with a `write`. Use `serif` only to set a quotation or a book title apart.

## Pacing

Viewers need time. Too fast is the most common fault. The next is a picture that stops: keep the subject changing. A stretch of more than about 8 s where only sentences and glows change is dead time, so give the subject something to do or cut the stretch.

| What happens | Duration |
|---|---|
| A `write` of a heading or a sentence | 1 to 2 s |
| A `write` of an equation | 1.5 to 3 s |
| A wait after a sentence | 1.5 s plus 0.3 s per word |
| A wait after a new equation or a finished diagram | 2 to 4 s |
| A change that carries the idea: a morph, move, reaction, camera move | 2 to 4 s |
| A `create` of a shape or axes | 1 to 2 s |
| A `recolor` to point at something | 0.3 to 0.6 s |
| Clearing the stage: fading everything out | 0.6 to 0.8 s |

### Fit the length

Add up the plan's durations before you write the scene file. An idea with one sentence and one change takes about 10 to 12 s: a 1.5 s write, a 4.5 s wait for a 10-word sentence, a 3 s change and a 0.7 s fade. So a one-minute video holds about five ideas. When the sum is too long, cut an idea or merge two, and keep the reading time. A video that runs a few seconds long is better than one the viewer cannot read. `validate` reports the scene's `duration`: check it against the plan.

## Layout

Keep everything at least 0.4 world units inside the frame's edge, which at the default camera is x from -7.36 to 7.36 and y from -4.14 to 4.14. `inspect` tells you each object's box in pixels.

- Put the main thing in the middle, large. It should fill at least half the frame's height or width between the heading and the sentence. An equation the lesson is about can be `size` 0.9 or more, and a small molecule `scale` 1.3 to 1.6.
- Use the whole frame. When the picture sits on one side, put its equation or readout on the other, rather than leave half the frame empty.
- Never hold an empty frame. Overlap the end of one idea with the start of the next (`motion.md`).
- Leave space. If three things fight for the middle, the idea has too many parts: split it.
- Place things you will add later now, in the plan, so nothing has to jump out of the way.
- What is nearer the camera covers what is farther. For flat objects in one plane, give the one that must show on top a slightly larger z, such as 0.05: text over a filled shape needs it, or a see-through fill tints the text. A trail draws at the depth of the object it follows, so put that object nearer to draw its trail over another.
- An object stays where it is until a step moves or fades it. Clear the stage between ideas: one step that fades out everything the next idea does not need.

## Color

Give each quantity one color and keep it for the whole video. If x is blue on the plot, x is blue in the equation and in the sentence about it. In math, color a term with `\textcolor{#58c4dd}{x}` (see `math.md`). Use white for text, gray (`#888888`) for what is in the background, and at most four colors for meaning.

At any moment, one object is the one the viewer should watch. Make it the brightest thing on screen, and keep the rest in their quantity colors or in gray. When the lesson moves on to another object, recolor the first back before the next one lights. To dim what is no longer the subject, recolor it to a darker shade of its color rather than fading it.

To mark a state, such as the element a sort compares or the node a search visits, `recolor` the object rather than stacking one copy per color: `{ "type": "recolor", "object": "box4", "stroke": "#ffd23f", "fill": "#4a3f14" }` for a flat shape, `"color"` for text, math or a sphere. `inspect` reports each object's `colors`.

These colors read well on the default background `#1e1e1e`:

| Color | Hex |
|---|---|
| blue | `#58c4dd` |
| green | `#83c167` |
| red | `#fc6255` |
| yellow | `#ffd23f` |
| purple | `#9a72ac` for shapes and glows, and `#b189c6` for text of any size: the darker one fails the layout check's contrast |
| gray | `#888888` |

## What the tool cannot do

Know these before you plan, so the plan works:

- Sound only from sound files on disk, each starting at a chapter, at the loudness it has (`narration.md`). The tool makes no voice or music itself.
- An object's text and shape never change. To change them, morph the object into another object, or fade one out and the other in. Its colors can change: see "Color".
- An object is created or written once. To bring it back after a fade out, fade it in again.
- No group of objects. To move several objects as one, attach the small ones to the large one (`math.md`), or give each the same animation. Attachment carries position, not rotation.
- A value changes by an easing curve toward a `to`, by a `follow` from a law or along a curve, or by a `simulate` from its rates (`motion.md`). Nothing else moves an object.

## Check your work

Follow the loop in `SKILL.md`. Look at each contact sheet and ask what no layout check can:

- Does the first frame show the thing the video is about?
- Does each checked scene time show one idea, with nothing left over from the last?
- Does the first scene time of each idea show the thing the sentence talks about?
- Is the subject large and in the middle, or is half the frame empty?
- Is any frame empty but for the heading?
- Can you tell which object to watch, by its brightness, in every tile?
- Does each sentence stay up long enough to read, by the pacing table?
- Is there a run of tiles, more than about 8 s, where only the text changes?
- Does each sentence that says why point at something on the picture that shows it?
- Is every glow, ring or mark of the last idea gone?
- Is anything shown that the lesson plan never mentions, such as an object declared but never faded in?

The layout check finds what is wrong with the picture: anything cut off, covering something else, too small, too faint, hidden, or passing through something. It checks text exactly beside a molecule (against the disc each ball shows on the frame, from sci_anim 0.11.0), but beside a cartoon against its box (from sci_anim 0.23.1), axes (against their lines, ticks and labels), a plot (against its stroke, glyph by glyph) and a pattern (against each copy that shows). A text wholly inside a filled shape is set on the fill, and it checks only the contrast. A text whose center lies in a corner of a filled circle's box, off its disc, is read against what lies behind the circle. A text over a solid or a sphere is read against its color and the darker shade of its side toward the camera. A faded text is read at its object opacity, as the frame shows it: its color mixed toward what is behind it. It is judged wherever its opacity rests, so a text faded to 0.2 and left there is too faint, while a fade in or a follow of opacity that passes through faint opacities is judged only from the end of its span. A text at opacity 0 is hidden and not judged. Beside any other shape, such as a line, a circle's stroke or a trail, it compares rectangles, so it can list a text the frame shows in empty space beside a slanted line, inside a circle, or under the arch of a trail. Move the text clear when you can. Pass `--allow <id>,<id>` only for an overlap you placed on purpose and have seen clear on a full-size frame, because an allow hides every fault of that pair, real or not.
