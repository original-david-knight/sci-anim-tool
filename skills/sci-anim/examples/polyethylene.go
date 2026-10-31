//go:build ignore

// Writes polyethylene.json, or the path given. Edit this, not the JSON. Run it from this folder:
//
//	go run polyethylene.go
//
// A methyl radical opens an ethylene's double bond, and the new radical end opens the next, so the
// viewer watches three ethylenes join into a chain while the first one stays lit as a unit of it.
// structures/ethylene.sdf is PubChem's, and structures/NOTICE says how the four radicals were
// written, in one shared set of positions.
package main

import (
	"encoding/json"
	"fmt"
	"math"
	"os"
	"reflect"
	"slices"
	"strconv"
	"strings"
)

const (
	yellow, green, blue = "#ffd23f", "#83c167", "#58c4dd"
	scale               = 1.2
	electronOffset      = 1.3 // world units from a radical end to the caption that marks its unpaired electron
)

var (
	chainNames = []string{"methyl_radical", "propyl_radical", "pentyl_radical", "heptyl_radical"}
	origin     = [2]float64{-4.8, -0.4} // world x and y of the chain files' origin
	approach   = [2]float64{1.2, 1.0}   // where an ethylene waits, in ångströms from the unit it becomes
)

type atom struct {
	element  string
	position []float64
}

// structure is a structure file's atoms, and its bonds as pairs of atom numbers from 1.
type structure struct {
	atoms []atom
	bonds [][2]int
}

func readStructure(name string) structure {
	data, err := os.ReadFile("structures/" + name + ".sdf")
	if err != nil {
		panic(err)
	}
	lines := strings.Split(string(data), "\n")
	field := func(line string, from, to int) string { return strings.TrimSpace(line[from:to]) }
	number := func(line string, from, to int) float64 {
		n, err := strconv.ParseFloat(field(line, from, to), 64)
		if err != nil {
			panic(err)
		}
		return n
	}
	atomCount, bondCount := int(number(lines[3], 0, 3)), int(number(lines[3], 3, 6))
	var s structure
	for _, line := range lines[4 : 4+atomCount] {
		s.atoms = append(s.atoms, atom{field(line, 31, 34), []float64{number(line, 0, 10), number(line, 10, 20), number(line, 20, 30)}})
	}
	for _, line := range lines[4+atomCount : 4+atomCount+bondCount] {
		s.bonds = append(s.bonds, [2]int{int(number(line, 0, 3)), int(number(line, 3, 6))})
	}
	return s
}

func (s structure) hydrogensOf(carbon int) []int {
	var found []int
	for _, b := range s.bonds {
		other := b[0]
		if other == carbon {
			other = b[1]
		} else if b[1] != carbon {
			continue
		}
		if s.atoms[other-1].element == "H" {
			found = append(found, other)
		}
	}
	return found
}

func (s structure) positions() [][]float64 {
	var p [][]float64
	for _, a := range s.atoms {
		p = append(p, a.position)
	}
	return p
}

func centroid(points [][]float64) []float64 {
	c := make([]float64, 3)
	for i := range c {
		for _, p := range points {
			c[i] += p[i]
		}
		c[i] /= float64(len(points))
	}
	return c
}

func world(p []float64) []float64 {
	return []float64{origin[0] + scale*p[0], origin[1] + scale*p[1], scale * p[2]}
}

func turned(p []float64, degrees float64) []float64 {
	a := degrees * math.Pi / 180
	return []float64{p[0]*math.Cos(a) - p[1]*math.Sin(a), p[0]*math.Sin(a) + p[1]*math.Cos(a), p[2]}
}

func distance(p, q []float64) float64 {
	return math.Sqrt((p[0]-q[0])*(p[0]-q[0]) + (p[1]-q[1])*(p[1]-q[1]) + (p[2]-q[2])*(p[2]-q[2]))
}

// round rounds to places decimals, as the decimal is written, so 0.125 rounds to 0.12.
func round(f float64, places int) float64 {
	r, _ := strconv.ParseFloat(strconv.FormatFloat(f, 'f', places, 64), 64)
	return r
}

func rounded(p []float64, places int) []float64 {
	var r []float64
	for _, v := range p {
		r = append(r, round(v, places))
	}
	return r
}

// unitAtoms is unit u of a chain: unit 1 is the chain's carbons 2 and 3, from the first ethylene,
// with their hydrogens.
func (s structure) unitAtoms(u int) []int {
	carbons := []int{2 * u, 2*u + 1}
	return append(carbons, append(s.hydrogensOf(carbons[0]), s.hydrogensOf(carbons[1])...)...)
}

var (
	ethylene = readStructure("ethylene")
	chains   []structure
)

func chainObject(k int, opacity any) obj {
	c := chains[k]
	groups := o("methyl_end", append([]int{1}, c.hydrogensOf(1)...), "radical_end", []int{2*k + 1})
	for u := 1; u <= k; u++ {
		groups.set(fmt.Sprintf("unit_%d", u), c.unitAtoms(u))
	}
	m := o("type", "molecule", "file", "structures/"+chainNames[k]+".sdf",
		"position", rounded(world(centroid(c.positions())), 4),
		"scale", []float64{scale, scale, scale},
		"groups", groups)
	if opacity != nil {
		m.set("opacity", opacity)
	}
	return m
}

// placedEthylene is the ethylene the chain with k units adds, waiting beside the unit it becomes,
// and the atom map of that reaction.
func placedEthylene(k int) (obj, []obj) {
	product := chains[k+1]
	newCarbons := []int{2*k + 2, 2*k + 3}
	var unit [][]float64
	for _, a := range product.unitAtoms(k + 1) {
		unit = append(unit, product.atoms[a-1].position)
	}
	first, second := product.atoms[newCarbons[0]-1].position, product.atoms[newCarbons[1]-1].position
	angle := math.Atan2(second[1]-first[1], second[0]-first[0]) * 180 / math.Pi
	middle := centroid(ethylene.positions())
	unitMiddle := centroid(unit)
	position := world([]float64{unitMiddle[0] + approach[0], unitMiddle[1] + approach[1], unitMiddle[2]})
	var rest [][]float64
	for _, a := range ethylene.atoms {
		t := turned([]float64{a.position[0] - middle[0], a.position[1] - middle[1], a.position[2] - middle[2]}, angle)
		rest = append(rest, []float64{position[0] + scale*t[0], position[1] + scale*t[1], position[2] + scale*t[2]})
	}
	// Ethylene's atom 1 lies at -x in its file, so the turn sets it nearer the chain: it bonds to the radical.
	pairs := map[int]int{1: newCarbons[0], 2: newCarbons[1]}
	for ethyleneCarbon, chainCarbon := range map[int]int{1: newCarbons[0], 2: newCarbons[1]} {
		mine, theirs := ethylene.hydrogensOf(ethyleneCarbon), product.hydrogensOf(chainCarbon)
		near := func(m, t int) float64 { return distance(rest[mine[m]-1], world(product.atoms[theirs[t]-1].position)) }
		if near(0, 0)+near(1, 1) > near(0, 1)+near(1, 0) {
			slices.Reverse(theirs)
		}
		for i := range mine {
			pairs[mine[i]] = theirs[i]
		}
	}
	ethyleneID, chainID, productID := fmt.Sprintf("ethylene_%d", k+1), fmt.Sprintf("chain_%d", k), fmt.Sprintf("chain_%d", k+1)
	row := func(reactant string, a int, b int) obj {
		return o("reactant", o("object", reactant, "atom", a), "product", o("object", productID, "atom", b))
	}
	var atomMap []obj
	for a, reactantAtom := range chains[k].atoms {
		var same []int
		for b, productAtom := range product.atoms {
			if productAtom.element == reactantAtom.element && distance(reactantAtom.position, productAtom.position) < 1e-3 {
				same = append(same, b+1)
			}
		}
		if len(same) != 1 {
			panic(fmt.Sprintf("%s atom %d is at %d places in %s", chainID, a+1, len(same), productID))
		}
		atomMap = append(atomMap, row(chainID, a+1, same[0]))
	}
	ethyleneAtoms := make([]int, 0, len(pairs))
	for a := range pairs {
		ethyleneAtoms = append(ethyleneAtoms, a)
	}
	slices.Sort(ethyleneAtoms)
	for _, a := range ethyleneAtoms {
		atomMap = append(atomMap, row(ethyleneID, a, pairs[a]))
	}
	placed := o("type", "molecule", "file", "structures/ethylene.sdf", "position", rounded(position, 4),
		"rotation", []any{0, 0, round(angle, 3)}, "scale", []float64{scale, scale, scale}, "opacity", 0,
		"groups", o("double_bond", []int{1, 2}))
	return placed, atomMap
}

// offsetOfElectron points from chain k's radical end toward where the next carbon bonds, on the frame.
func offsetOfElectron(k int) []float64 {
	atoms := chains[k].atoms
	end := atoms[2*k].position
	var toward []float64
	if k+1 < len(chains) {
		toward = chains[k+1].atoms[2*k+1].position
	} else {
		before, last := atoms[2*k-2].position, atoms[2*k-1].position
		toward = []float64{end[0] + last[0] - before[0], end[1] + last[1] - before[1], end[2] + last[2] - before[2]}
	}
	dx, dy := toward[0]-end[0], toward[1]-end[1]
	length := math.Hypot(dx, dy)
	return []float64{round(electronOffset*dx/length, 3), round(electronOffset*dy/length, 3)}
}

func text(s string, y, size any) obj {
	return o("type", "text", "text", s, "size", size, "position", []any{0, y, 0})
}

func say(s string) obj { return text(s, -3.4, 0.4) }

func write(object string) obj { return o("type", "write", "object", object) }

func fade(object string, to any) obj { return o("type", "fade", "object", object, "to", to) }

func light(object, group string, to any, color string) obj {
	h := o("type", "highlight", "object", object, "group", group, "to", to)
	if color != "" {
		h.set("color", color)
	}
	return h
}

func reaction(reactants, products []string, atomMap []obj) obj {
	return o("type", "reaction", "reactants", reactants, "products", products, "atom_map", atomMap, "fit", false)
}

func play(duration any, animations ...obj) obj {
	return o("type", "play", "duration", duration, "animations", animations)
}

func wait(duration any) obj { return o("type", "wait", "duration", duration) }

func main() {
	for k, name := range chainNames {
		chain := readStructure(name)
		carbons := 0
		for _, a := range chain.atoms {
			if a.element == "C" {
				carbons++
			}
		}
		if carbons != 2*k+1 {
			panic(fmt.Sprintf("%s has %d carbons, not %d", name, carbons, 2*k+1))
		}
		chains = append(chains, chain)
	}

	objects := o("heading", text("How ethylene becomes polyethylene", 3.3, 0.5))
	objects.set("chain_0", chainObject(0, 0))
	for k := 1; k < len(chains); k++ {
		objects.set(fmt.Sprintf("chain_%d", k), chainObject(k, nil))
	}
	var atomMaps [][]obj
	for k := 0; k < len(chains)-1; k++ {
		placed, atomMap := placedEthylene(k)
		objects.set(fmt.Sprintf("ethylene_%d", k+1), placed)
		atomMaps = append(atomMaps, atomMap)
	}
	for k := range chains {
		electron := o("type", "text", "text", "•", "size", 0.9, "color", blue)
		if k == 0 {
			electron.set("opacity", 0)
		}
		electron.set("attach", o("object", fmt.Sprintf("chain_%d", k), "group", "radical_end", "offset", offsetOfElectron(k)))
		objects.set(fmt.Sprintf("electron_%d", k), electron)
	}
	objects = append(objects, o(
		"say_radical", say("An initiator gives a methyl radical, with one unpaired electron."),
		"say_ethylene", say("Ethylene has a double bond between its two carbons."),
		"say_opens", say("The radical opens the double bond and bonds to one carbon."),
		"say_moves", say("The unpaired electron moves to the far carbon."),
		"say_unit", say("That ethylene is now one unit of the chain."),
		"say_grows", say("Each new radical end opens the next ethylene."),
		"say_same", say("Every unit is the same: CH₂–CH₂."),
		"say_ends", say("Its ends: CH₃ from the initiator, and the radical end."),
		"say_real", say("A real chain grows thousands of units, until two radicals meet."),
		"scheme", o("type", "math", "tex", `n\,\mathrm{CH_2{=}CH_2} \rightarrow \mathrm{-(CH_2{-}CH_2)_{\mathit{n}}-}`,
			"size", 0.7, "position", []any{0, 2.2, 0}),
	)...)

	timeline := []obj{
		play(1.5, write("heading")),
		play(1.2, fade("chain_0", 1), fade("electron_0", 1), write("say_radical")),
		wait(4),
		play(0.5, fade("say_radical", 0)),

		play(1.2, fade("ethylene_1", 1), write("say_ethylene"), light("ethylene_1", "double_bond", 1, yellow)),
		wait(3),
		play(0.6, fade("say_ethylene", 0), light("ethylene_1", "double_bond", 0, "")),
		play(1.5, write("say_opens")),
		play(3.5, reaction([]string{"chain_0", "ethylene_1"}, []string{"chain_1"}, atomMaps[0])),
		wait(1.5),
		play(0.5, fade("say_opens", 0)),
		play(1.2, write("say_moves")),
		wait(2.5),
		play(0.5, fade("say_moves", 0)),
		play(1.2, write("say_unit"), light("chain_1", "unit_1", 1, green)),
		wait(2.5),
		play(0.5, fade("say_unit", 0)),

		play(1.2, fade("ethylene_2", 1), write("say_grows")),
		play(3, reaction([]string{"chain_1", "ethylene_2"}, []string{"chain_2"}, atomMaps[1])),
		play(1, fade("ethylene_3", 1)),
		play(3, reaction([]string{"chain_2", "ethylene_3"}, []string{"chain_3"}, atomMaps[2])),
		wait(1.5),
		play(0.5, fade("say_grows", 0), light("chain_3", "unit_1", 0, ""), fade("electron_3", 0)),

		play(1.2, write("say_same"), light("chain_3", "unit_1", 1, green)),
		play(0.8, light("chain_3", "unit_1", 0, ""), light("chain_3", "unit_2", 1, green)),
		play(0.8, light("chain_3", "unit_2", 0, ""), light("chain_3", "unit_3", 1, green)),
		play(0.8, light("chain_3", "unit_3", 0, "")),
		play(2, write("scheme")),
		wait(2),
		play(0.5, fade("say_same", 0)),
		play(1.5, write("say_ends"), fade("electron_3", 1),
			light("chain_3", "methyl_end", 1, blue), light("chain_3", "radical_end", 1, blue)),
		wait(3.5),
		play(0.5, fade("say_ends", 0), light("chain_3", "methyl_end", 0, ""), light("chain_3", "radical_end", 0, "")),
		play(1.5, write("say_real")),
		wait(3.5),
	}

	scene := o("objects", objects, "timeline", timeline)
	out := "polyethylene.json"
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

// encode writes v as JSON with one space of indent a level. A float64 keeps its decimal point,
// so 2.0 stays 2.0 while the int 2 stays 2.
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
		s := strconv.FormatFloat(v, 'f', -1, 64)
		if !strings.Contains(s, ".") {
			s += ".0"
		}
		b.WriteString(s)
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
