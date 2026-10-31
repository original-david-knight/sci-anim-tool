//go:build ignore

// Writes unzipping_ladder.json, or the path given. Edit this, not the JSON. Run it from this
// folder:
//
//	go run unzipping_ladder.go
//
// A short stretch of DNA as a ladder whose rungs unzip one after another, as a helicase opens
// the double helix: each rung's two halves, copies of two patterns colored by their sequence,
// slide apart with their bases' letters, from the top rung down. Then one codon of the left
// strand lights up, its three copies lit by a highlight each. The two strands pair A with T and
// G with C.
package main

import (
	"encoding/json"
	"fmt"
	"os"
	"reflect"
	"strconv"
	"strings"
)

const (
	strand  = "ATGCGTA" // the left strand, bottom rung first
	spacing = 0.8       // between rungs
	half    = 0.8       // from the middle to a rung half's center, before it slides
	slid    = 2.0       // and after
	letterX = 2.15      // from the middle to a base's letter, before it slides
	gray    = "#888888"
	lag     = 0.5 // between one rung starting to unzip and the next
	slide   = 1.0 // how long a rung takes to unzip
)

// Each base's palette index, in the palette's order: adenine, thymine, guanine, cytosine.
var paletteIndex = map[byte]int{'A': 0, 'T': 1, 'G': 2, 'C': 3}

var partner = map[byte]byte{'A': 'T', 'T': 'A', 'G': 'C', 'C': 'G'}

// The codon lit at the end: the left strand's rungs 1 to 3, TGC, which codes for cysteine.
var codon = []int{1, 2, 3}

func main() {
	var left, right []byte
	for k := 0; k < len(strand); k++ {
		left = append(left, strand[k])
		right = append(right, partner[strand[k]])
	}
	objects := o(
		"title", text("A ladder unzips rung by rung", 0, 3.55, 0.42, "#ffffff"),
		"left_nucleobases", halves(left, -half),
		"right_nucleobases", halves(right, half),
	)
	var unzip []obj
	for k := range left {
		y := rungY(k)
		objects = append(objects,
			field{fmt.Sprintf("left_%d", k), text(string(left[k]), -letterX, y, 0.4, "#ffffff")},
			field{fmt.Sprintf("right_%d", k), text(string(right[k]), letterX, y, 0.4, "#ffffff")})
		// The top rung opens first, as a helicase walks down the helix.
		start := float64(len(left)-1-k) * lag
		span := []any{"start", start, "end", start + slide}
		unzip = append(unzip,
			animation("move", "left_nucleobases", append([]any{"copy", []any{0, k, 0}, "to", []any{half - slid, y, 0}}, span...)...),
			animation("move", "right_nucleobases", append([]any{"copy", []any{0, k, 0}, "to", []any{slid - half, y, 0}}, span...)...),
			animation("move", fmt.Sprintf("left_%d", k), append([]any{"to", []any{-letterX - slid + half, y, 0}}, span...)...),
			animation("move", fmt.Sprintf("right_%d", k), append([]any{"to", []any{letterX + slid - half, y, 0}}, span...)...))
	}
	var lit []obj
	for _, k := range codon {
		lit = append(lit, animation("highlight", "left_nucleobases", "copy", []any{0, k, 0}, "to", 1))
	}
	var create []obj
	create = append(create,
		animation("create", "left_nucleobases", "easing_curve", "linear"),
		animation("create", "right_nucleobases", "easing_curve", "linear"))
	for k := range left {
		create = append(create, animation("write", fmt.Sprintf("left_%d", k)), animation("write", fmt.Sprintf("right_%d", k)))
	}
	scene := o(
		"objects", objects,
		"timeline", []obj{
			play(1.0, animation("write", "title")),
			play(2.0, create...),
			o("type", "wait", "duration", 0.5),
			play(float64(len(left)-1)*lag+slide, unzip...),
			play(1.0, lit...),
			o("type", "wait", "duration", 1.5),
		},
	)
	out := "unzipping_ladder.json"
	if len(os.Args) > 1 {
		out = os.Args[1]
	}
	if err := os.WriteFile(out, []byte(encode(scene)+"\n"), 0o644); err != nil {
		panic(err)
	}
}

// The y of rung k, the middle rung at 0.
func rungY(k int) float64 {
	return float64(k-len(strand)/2) * spacing
}

// One strand's rung halves: a pattern of one line a rung, each colored by its base. A copy's
// position is in the pattern's own space, where every copy's x is 0 before it slides.
func halves(bases []byte, x float64) obj {
	var indices []any
	for _, b := range bases {
		indices = append(indices, paletteIndex[b])
	}
	return o(
		"type", "pattern",
		"shape", o("type", "line", "from", []any{-0.72, 0}, "to", []any{0.72, 0}, "stroke", o("color", gray, "width", 0.2)),
		"layout", o("type", "grid", "counts", []any{1, len(bases), 1}, "spacing", spacing),
		"copy", o("palette", []any{"#fc6255", "#ffd23f", "#83c167", "#58c4dd"}, "palette_index", indices),
		"position", []any{x, 0, 0},
	)
}

func text(words string, x, y, size float64, color string) obj {
	return o("type", "text", "text", words, "size", size, "color", color, "position", []any{x, y, 0})
}

func animation(kind, object string, rest ...any) obj {
	return append(o("type", kind, "object", object), o(rest...)...)
}

func play(duration float64, animations ...obj) obj {
	return o("type", "play", "duration", duration, "animations", animations)
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
		made = append(made, field{keysAndValues[i].(string), keysAndValues[i+1]})
	}
	return made
}

// num writes a float64 with a decimal point, so 2.0 stays 2.0 while the int 2 stays 2.
func num(f float64) string {
	s := strconv.FormatFloat(f, 'f', -1, 64)
	if !strings.Contains(s, ".") {
		s += ".0"
	}
	return s
}

// encode writes v as JSON with one space of indent a level, a row of a table on one line.
func encode(v any) string {
	var b strings.Builder
	writeJSON(&b, v, 0)
	return b.String()
}

func writeJSON(b *strings.Builder, v any, depth int) {
	newline := func(depth int) { b.WriteString("\n" + strings.Repeat(" ", depth)) }
	switch v := v.(type) {
	case obj:
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
	default:
		list := reflect.ValueOf(v)
		if list.Kind() != reflect.Slice {
			panic(fmt.Sprintf("cannot write %T as JSON", v))
		}
		flat := true
		for i := 0; i < list.Len(); i++ {
			kind := reflect.ValueOf(list.Index(i).Interface()).Kind()
			flat = flat && kind != reflect.Slice && kind != reflect.Map
			if _, isObj := list.Index(i).Interface().(obj); isObj {
				flat = false
			}
		}
		if flat {
			b.WriteString("[")
			for i := 0; i < list.Len(); i++ {
				if i > 0 {
					b.WriteString(", ")
				}
				writeJSON(b, list.Index(i).Interface(), depth+1)
			}
			b.WriteString("]")
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
