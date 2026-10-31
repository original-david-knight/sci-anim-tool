//go:build ignore

// Writes polymer_coil.json, or the path given. Edit this, not the JSON. Run it from this folder:
//
//	go run polymer_coil.go
//
// A polymer chain is a tangled coil, not the straight zigzag a structure drawing shows. A seeded
// random walk that never comes back near itself writes the curve points of the chain's backbone,
// a tube is drawn through them, and an arrowhead rides the chain from one end to the other.
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
	blue, yellow = "#58c4dd", "#ffd23f"
	seed         = 20261027
	beads        = 300
)

// The walk. These are variables, not constants, because Go computes constant arithmetic exactly,
// and the committed file has float64's rounding.
var (
	bond  = 0.27                        // the distance from one curve point to the next
	bend  = 0.8                         // how far each step may turn from the one before
	apart = 0.3                         // how close the chain may come to itself, beyond its neighbors
	room  = [3]float64{4.6, 2.3, 1.8}   // the half-sizes of the ellipsoid the coil fills
	start = [3]float64{-4.0, -0.3, 0.0} // the first curve point
)

// rng is xorshift64*, so the walk is the same on every machine and every Go version.
type rng uint64

// next is a number from -1 to 1.
func (r *rng) next() float64 {
	*r ^= *r >> 12
	*r ^= *r << 25
	*r ^= *r >> 27
	return float64(uint64(*r)*2685821657736338717>>11)/float64(1<<52) - 1
}

type vec [3]float64

// Each product is converted on its own, so a build that fuses a multiply and an add, as arm64's
// may, writes the same file.
func (a vec) plus(b vec, by float64) vec {
	return vec{a[0] + float64(b[0]*by), a[1] + float64(b[1]*by), a[2] + float64(b[2]*by)}
}

func (a vec) length() float64 {
	return math.Sqrt(float64(a[0]*a[0]) + float64(a[1]*a[1]) + float64(a[2]*a[2]))
}

func (a vec) unit() vec { return vec{}.plus(a, 1/a.length()) }

// inside says whether p lies in the ellipsoid of half-sizes room about the origin.
func inside(p vec) bool {
	sum := 0.0
	for k := range p {
		sum += float64(p[k] / room[k] * p[k] / room[k])
	}
	return sum <= 1
}

// rounded keeps three decimals, which is what the scene file writes.
func rounded(p vec) vec {
	for k := range p {
		p[k] = math.Round(p[k]*1000) / 1000
	}
	return p
}

// walk returns the chain's curve points. Its first step is along +x, so the first chord runs
// along +x and an object carried with `faces: tangent` starts unturned.
func walk() []vec {
	r := rng(seed)
	points := []vec{rounded(start), rounded(vec(start).plus(vec{1, 0, 0}, bond))}
	way := vec{1, 0, 0}
	for len(points) < beads {
		last := points[len(points)-1]
		placed := false
		for try := 0; try < 400 && !placed; try++ {
			turn := bend
			if try > 200 {
				turn = 2.5
			}
			next := way.plus(vec{r.next(), r.next(), r.next()}, turn).unit()
			p := rounded(last.plus(next, bond))
			if inside(p) && farFromAll(p, points[:len(points)-2]) {
				points, way, placed = append(points, p), next, true
			}
		}
		if !placed {
			panic(fmt.Sprintf("the walk is stuck at curve point %d; change the seed or give it more room", len(points)))
		}
	}
	return points
}

func farFromAll(p vec, others []vec) bool {
	for _, q := range others {
		if p.plus(q, -1).length() < apart {
			return false
		}
	}
	return true
}

func text(s string, y, size float64) obj {
	return o("type", "text", "text", s, "size", size, "position", []any{0, y, 0}, "opacity", 0)
}

func animation(kind, object string, fields ...any) obj {
	return append(o("type", kind, "object", object), o(fields...)...)
}

func play(duration float64, animations ...obj) obj {
	return o("type", "play", "duration", duration, "animations", animations)
}

func main() {
	chain := walk()
	curvePoints := make([]any, len(chain))
	for i, p := range chain {
		curvePoints[i] = point(p)
	}
	scene := o(
		"camera", o("position", []any{0, 1.2, 11}, "target", []any{0, 0, 0}),
		"objects", o(
			"heading", text("A polymer chain is a tangled coil", 3.4, 0.45),
			"chain", o("type", "tube", "radius", 0.06,
				"curve", o("points", curvePoints),
				"material", o("type", "standard", "color", blue)),
			"rider", o("type", "prism", "position", []any{chain[0][0], chain[0][1], chain[0][2]}, "height", 0.18,
				"base", o("type", "polygon", "points", []any{[]any{0.45, 0}, []any{-0.22, 0.24}, []any{-0.22, -0.24}}),
				"material", o("type", "standard", "color", yellow), "opacity", 0),
			"say", text("One chain of 300 units, coiled and folded on itself", -3.4, 0.36),
		),
		"timeline", []obj{
			play(1.0, animation("fade", "heading", "to", 1)),
			play(3.0, animation("create", "chain", "easing_curve", "linear")),
			play(0.6, animation("fade", "rider", "to", 1)),
			play(8.0,
				animation("follow", "rider", "along", "chain", "faces", "tangent", "easing_curve", "linear"),
				animation("fade", "say", "to", 1, "start", 0, "end", 1)),
			o("type", "wait", "duration", 1.0),
		},
	)
	out := "polymer_coil.json"
	if len(os.Args) > 1 {
		out = os.Args[1]
	}
	if err := os.WriteFile(out, []byte(encode(scene)+"\n"), 0o644); err != nil {
		panic(err)
	}
}

// point is a curve point, written on one line.
type point vec

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
	case point:
		b.WriteString("[" + num(v[0]) + ", " + num(v[1]) + ", " + num(v[2]) + "]")
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
