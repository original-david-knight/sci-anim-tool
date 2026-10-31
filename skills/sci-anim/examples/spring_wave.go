//go:build ignore

// Writes spring_wave.json, or the path given. Edit this, not the JSON. Run it from this folder:
//
//	go run spring_wave.go
//
// A mass bounces on a spring while a point plots its height over time, so the viewer watches a
// sine wave being drawn, then reads its amplitude and its period off the same picture.
package main

import (
	"encoding/json"
	"fmt"
	"math"
	"os"
	"reflect"
	"strconv"
	"strings"
)

const (
	blue, green, yellow, gray = "#58c4dd", "#83c167", "#ffd23f", "#888888"
	dimBlue                   = "#2f6878"
)

// The graph: time in seconds along x, height in cm along y. These are variables, not constants,
// because Go computes constant arithmetic exactly, and the committed file has float64's rounding.
var (
	x0, sx   = -3.4, 1.1 // world x of t = 0, and world units per second
	sy       = 0.12      // world units per cm
	tMax     = 8.6
	ampCM    = 10
	amp      = float64(ampCM) * sy
	massX    = -5.3
	ceilingY = 2.85
	half     = 0.35 // half the mass's side
)

// float64() rounds the product before the sum, so a build that fuses the two, as arm64's may,
// writes the same file.
func wx(t float64) float64 { return x0 + float64(sx*t) }

var massLaw = o("x", num(massX), "y", "-"+num(amp)+"*cos(pi*t)", "z", "0.02")

func text(s string, x, y, size any, color string) obj {
	t := o("type", "text", "text", s, "size", size, "position", []any{x, y, 0})
	if color != "" {
		t.set("color", color)
	}
	return t
}

func say(s string) obj { return text(s, 0, -3.4, 0.4, "") }

func heading(s string) obj { return text(s, 0, 3.3, 0.5, "") }

// A dashed line is a line whose stroke has a dash pattern.
func dashes(xFrom, xTo float64, y any, color string, opacity any) obj {
	d := o("type", "line", "from", []any{xFrom, 0}, "to", []any{xTo, 0}, "position", []any{0, y, 0},
		"stroke", o("color", color, "width", 0.025, "dash", []any{0.1, 0.1}))
	if opacity != nil {
		d.set("opacity", opacity)
	}
	return d
}

func bracket(t0, t1 float64) obj {
	a, b := wx(t0)+0.05, wx(t1)-0.05
	return o("type", "path", "d", fmt.Sprintf("M %.3f -1.42 L %.3f -1.6 L %.3f -1.6 L %.3f -1.42", a, a, b, b),
		"stroke", o("color", green, "width", 0.05))
}

func objects() obj {
	return o(
		"heading", heading("How a spring draws a wave"),
		"heading_amplitude", heading("Amplitude"),
		"heading_period", heading("Period"),
		"heading_summary", heading("Amplitude and period"),

		"ceiling", o("type", "line", "from", []any{-0.85, 0}, "to", []any{0.85, 0}, "position", []any{massX, ceilingY, 0},
			"stroke", o("color", gray, "width", 0.06)),
		"hatch", o("type", "pattern",
			"shape", o("type", "line", "from", []any{-0.08, -0.08}, "to", []any{0.08, 0.08}, "stroke", o("color", gray, "width", 0.025)),
			"layout", o("type", "grid", "counts", []any{8, 1, 1}, "spacing", 0.2),
			"position", []any{massX, ceilingY + 0.12, 0}),
		"mass", o("type", "polygon", "points", []any{[]any{-half, -half}, []any{half, -half}, []any{half, half}, []any{-half, half}},
			"position", []any{massX, 0, 0.02}, "fill", o("color", blue), "stroke", o("color", "#a8e4f2", "width", 0.03), "opacity", 0),
		"spring", o("type", "spring", "from", []any{massX, ceilingY}, "to", o("object", "mass", "offset", []any{0, half, 0}),
			"turns", 8, "radius", 0.22, "tube_radius", 0.03, "material", o("type", "standard", "color", "#cccccc")),
		"rest", dashes(-5.9, wx(0)-0.1, 0, gray, nil),
		"rest_caption", text("rest", -6.5, 0, 0.3, gray),

		"ax", o("type", "axes",
			"x", o("range", []any{0, tMax}, "spacing", 1, "label", `t\ (\text{s})`),
			"y", o("range", []any{-15, 15}, "spacing", 40, "label", `\text{height}`),
			"width", sx*tMax, "height", sy*30, "position", []any{x0 + sx*tMax/2, 0, 0}),
		"height_point", o("type", "dot", "axes", "ax", "position", []any{0, -ampCM, 0}, "radius", 0.09, "fill", o("color", "#ffffff"), "opacity", 0),
		"height_trail", o("type", "trail", "object", "height_point", "stroke", o("color", blue, "width", 0.06)),
		"level_line", o("type", "line", "from", o("object", "mass", "offset", []any{half, 0, 0}), "to", o("object", "height_point"),
			"stroke", o("color", gray, "width", 0.025), "opacity", 0),

		"amplitude_top", dashes(-5.9, wx(8.3), amp, yellow, 0),
		"amplitude_bottom", dashes(-5.9, wx(8.3), -amp, yellow, 0),
		"amplitude_at_spring", o("type", "arrow", "from", []any{-6.05, 0}, "to", []any{-6.05, amp}, "tip_length", 0.2,
			"stroke", o("color", yellow, "width", 0.05)),
		"amplitude_at_spring_caption", o("type", "math", "tex", "A", "size", 0.45, "color", yellow, "position", []any{-6.45, 0.6, 0}),
		"amplitude_on_axes", o("type", "arrow", "from", []any{wx(5), 0}, "to", []any{wx(5), amp}, "tip_length", 0.2,
			"stroke", o("color", yellow, "width", 0.05)),
		"amplitude_on_axes_caption", o("type", "math", "tex", `A = 10\text{ cm}`, "size", 0.45, "color", yellow, "position", []any{wx(5), amp + 0.6, 0}),

		// Nearer the camera than the blue trail, so the green cycle draws over it.
		"cycle_point", o("type", "dot", "axes", "ax", "position", []any{2, -ampCM, 0.05}, "radius", 0.1, "fill", o("color", green), "opacity", 0),
		"cycle_trail", o("type", "trail", "object", "cycle_point", "stroke", o("color", green, "width", 0.09)),
		"cycle_line", o("type", "line", "from", o("object", "mass", "offset", []any{half, 0, 0}), "to", o("object", "cycle_point"),
			"stroke", o("color", green, "width", 0.03), "opacity", 0),
		"period_equals", o("type", "math", "tex", "T =", "size", 0.45, "color", green, "align", "right",
			"position", []any{wx(3) - 0.35, -2.1, 0}, "opacity", 0),
		"stopwatch", o("type", "readout", "decimals", 1, "unit", `\text{ s}`, "size", 0.45, "color", green,
			"position", []any{wx(3) + 0.05, -2.1, 0}, "opacity", 0),
		"bracket_1", bracket(2, 4),
		"bracket_2", bracket(4, 6),
		"bracket_3", bracket(6, 8),

		"say_rest", say("A mass hangs on a spring, perfectly still."),
		"say_pull", say("Pull it down 10 cm and hold it there."),
		"say_plot", say("A point will plot its height at every moment."),
		"say_go", say("Let go!"),
		"say_wave", say("Up, down, up, down: the point draws a wave."),
		"say_sine", say("This smooth, repeating shape is called a sine wave."),
		"say_turn", say("The mass turns back at the same height every time."),
		"say_amp", say("Amplitude is how far it moves from rest: 10 cm."),
		"say_time", say("Now let's time one full cycle: up, then back down."),
		"say_period", say("Period is the time for one full cycle: 2 seconds."),
		"say_every", say("Every cycle takes the same 2 seconds."),
		"say_sum", say("Amplitude: how far it moves. Period: how long a cycle takes."),
	)
}

func animation(kind, object string, fields ...any) obj {
	return append(o("type", kind, "object", object), o(fields...)...)
}

// over gives an animation its span within its step.
func (a obj) over(start, end any) obj {
	a.set("start", start)
	a.set("end", end)
	return a
}

func fade(object string, to any) obj { return animation("fade", object, "to", to) }

func play(duration float64, animations ...obj) obj {
	return o("type", "play", "duration", duration, "animations", animations)
}

func wait(duration float64) obj { return o("type", "wait", "duration", duration) }

var intro = []obj{
	play(1.5, animation("write", "heading"), animation("create", "ceiling"), animation("create", "hatch")),
	play(1.5, animation("create", "spring"), fade("mass", 1), animation("create", "rest"), animation("write", "rest_caption").over(0.5, 1.5)),
	play(1.5, animation("write", "say_rest")),
	wait(2.0),
	play(0.6, fade("say_rest", 0)),
	play(2.0, animation("move", "mass", "to", []any{massX, -amp, 0.02}).over(0, 1.6), animation("write", "say_pull").over(0.2, 1.6)),
	wait(1.5),
	play(0.6, fade("say_pull", 0)),
	play(2.0, animation("create", "ax")),
	play(2.0, fade("height_point", 1).over(0, 0.6), fade("level_line", 1).over(0, 0.8), animation("write", "say_plot").over(0.3, 1.8)),
	wait(2.0),
	play(0.6, fade("say_plot", 0)),
}

// bounce holds the steps of the bounce chapter. The mass never stops, so every step follows it,
// and a wait step, which would hold it still, is never used.
type bounce struct {
	steps   []obj
	lawTime float64
}

func (b *bounce) step(duration float64, animations ...obj) {
	follow := o("type", "follow", "object", "mass", "law", massLaw, "from", "bounce")
	b.steps = append(b.steps, play(duration, append([]obj{follow}, animations...)...))
	b.lawTime += duration
}

func bounceSteps() []obj {
	var b bounce
	b.step(8.0,
		o("type", "follow", "object", "height_point", "law", o("x", "t", "y", fmt.Sprintf("-%d*cos(pi*t)", ampCM), "z", "0"), "from", "bounce"),
		animation("write", "say_go").over(0, 0.6), fade("say_go", 0).over(2.0, 2.5),
		animation("write", "say_wave").over(2.7, 4.2))
	b.step(4.5, fade("height_point", 0).over(0, 0.6), fade("level_line", 0).over(0, 0.6), fade("say_wave", 0).over(0, 0.6),
		animation("write", "say_sine").over(0.8, 2.3))
	b.step(3.0, fade("say_sine", 0).over(0, 0.6), fade("heading", 0).over(0, 0.6), animation("write", "heading_amplitude").over(0.8, 1.8),
		fade("amplitude_top", 0.8).over(1.0, 2.5), fade("amplitude_bottom", 0.8).over(1.0, 2.5))
	b.step(4.5, animation("write", "say_turn").over(0, 1.5))
	b.step(5.0, fade("say_turn", 0).over(0, 0.5),
		animation("create", "amplitude_at_spring").over(0.6, 1.6), animation("create", "amplitude_on_axes").over(0.6, 1.6),
		animation("write", "amplitude_at_spring_caption").over(1.2, 2.0), animation("write", "amplitude_on_axes_caption").over(1.2, 2.2),
		animation("write", "say_amp").over(2.4, 3.9))
	b.step(3.0)
	// The cycle replay must begin with the mass at the bottom, which is at every even law time.
	pad := math.Mod(2-math.Mod(b.lawTime+4.0, 2), 2)
	b.step(4.0+pad, fade("say_amp", 0).over(0, 0.5), fade("heading_amplitude", 0).over(0, 0.5), animation("write", "heading_period").over(0.6, 1.6),
		fade("amplitude_top", 0).over(0, 0.6), fade("amplitude_bottom", 0).over(0, 0.6),
		animation("recolor", "height_trail", "stroke", dimBlue).over(1.0, 1.6),
		animation("write", "say_time").over(1.4, 2.9))
	if math.Abs(math.Mod(b.lawTime, 2)) >= 1e-9 {
		panic(fmt.Sprintf("the cycle replay starts at law time %v, not at an even one", b.lawTime))
	}
	b.step(2.0,
		fade("cycle_point", 1).over(0, 0.1), fade("cycle_line", 1).over(0, 0.2),
		o("type", "follow", "object", "cycle_point", "law", o("x", "2 + t", "y", fmt.Sprintf("-%d*cos(pi*(2 + t))", ampCM), "z", "0.05")),
		fade("period_equals", 1).over(0, 0.2), fade("stopwatch", 1).over(0, 0.2),
		o("type", "follow", "object", "stopwatch", "sets", "value", "law", o("value", "t")))
	b.step(2.0, animation("create", "bracket_1").over(0, 0.8), fade("cycle_point", 0).over(0, 0.6), fade("cycle_line", 0).over(0, 0.6))
	b.step(4.5, fade("say_time", 0).over(0, 0.5), animation("write", "say_period").over(0.6, 2.1))
	b.step(4.5, fade("say_period", 0).over(0, 0.5), animation("write", "say_every").over(0.6, 1.8),
		animation("create", "bracket_2").over(1.2, 2.0), animation("create", "bracket_3").over(2.0, 2.8))
	b.step(3.0, fade("say_every", 0).over(0, 0.5), fade("heading_period", 0).over(0, 0.5), animation("write", "heading_summary").over(0.6, 1.6),
		animation("recolor", "height_trail", "stroke", blue).over(0, 0.6),
		animation("write", "say_sum").over(1.2, 2.7))
	b.step(5.0)
	var fades []obj
	for _, id := range []string{"heading_summary", "ceiling", "hatch", "mass", "spring", "rest", "rest_caption", "ax", "height_trail",
		"cycle_trail", "amplitude_at_spring", "amplitude_at_spring_caption", "amplitude_on_axes", "amplitude_on_axes_caption", "period_equals",
		"stopwatch", "bracket_1", "bracket_2", "bracket_3", "say_sum"} {
		fades = append(fades, fade(id, 0))
	}
	b.step(1.0, fades...)
	return b.steps
}

func main() {
	scene := o(
		"objects", objects(),
		"timeline", []obj{
			o("type", "chapter", "id", "intro", "steps", intro),
			o("type", "chapter", "id", "bounce", "steps", bounceSteps()),
		},
	)
	out := "spring_wave.json"
	if len(os.Args) > 1 {
		out = os.Args[1]
	}
	if err := os.WriteFile(out, []byte(encode(scene)+"\n"), 0o644); err != nil {
		panic(err)
	}
}

// obj is a JSON object that keeps its keys in the order they were set, so the scene file reads
// in the order the generator builds it.
type obj []field

type field struct {
	key   string
	value any
}

// o makes an object from keys and values in turn.
func o(keysAndValues ...any) obj {
	var made obj
	for i := 0; i < len(keysAndValues); i += 2 {
		made.set(keysAndValues[i].(string), keysAndValues[i+1])
	}
	return made
}

func (m *obj) set(key string, value any) {
	for i := range *m {
		if (*m)[i].key == key {
			(*m)[i].value = value
			return
		}
	}
	*m = append(*m, field{key, value})
}

// num writes a float64 with a decimal point, so 2.0 stays 2.0 while the int 2 stays 2.
func num(f float64) string {
	s := strconv.FormatFloat(f, 'f', -1, 64)
	if !strings.Contains(s, ".") {
		s += ".0"
	}
	return s
}

// encode writes v as JSON with one space of indent a level.
func encode(v any) string {
	var b strings.Builder
	writeJSON(&b, v, 0)
	return b.String()
}

func writeJSON(b *strings.Builder, v any, depth int) {
	newline := func(depth int) { b.WriteString("\n" + strings.Repeat(" ", depth)) }
	switch v := v.(type) {
	case obj:
		if len(v) == 0 {
			b.WriteString("{}")
			return
		}
		b.WriteString("{")
		for i, f := range v {
			if i > 0 {
				b.WriteString(",")
			}
			newline(depth + 1)
			writeJSON(b, f.key, depth+1)
			b.WriteString(": ")
			writeJSON(b, f.value, depth+1)
		}
		newline(depth)
		b.WriteString("}")
	case string:
		var s strings.Builder
		e := json.NewEncoder(&s)
		e.SetEscapeHTML(false)
		if err := e.Encode(v); err != nil {
			panic(err)
		}
		b.WriteString(strings.TrimSuffix(s.String(), "\n"))
	case float64:
		b.WriteString(num(v))
	case int:
		b.WriteString(strconv.Itoa(v))
	case bool:
		b.WriteString(strconv.FormatBool(v))
	default:
		list := reflect.ValueOf(v)
		if list.Kind() != reflect.Slice {
			panic(fmt.Sprintf("cannot write %T as JSON", v))
		}
		if list.Len() == 0 {
			b.WriteString("[]")
			return
		}
		b.WriteString("[")
		for i := 0; i < list.Len(); i++ {
			if i > 0 {
				b.WriteString(",")
			}
			newline(depth + 1)
			writeJSON(b, list.Index(i).Interface(), depth+1)
		}
		newline(depth)
		b.WriteString("]")
	}
}
