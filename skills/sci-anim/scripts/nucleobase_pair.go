//go:build ignore

// Writes the structure file of one Watson-Crick nucleobase pair, A-T or G-C, from idealized
// geometry:
//
//	go run nucleobase_pair.go <AT|GC> <file.sdf>
//
// The heavy atoms are the standard nucleobase geometry of Olson et al. (2001), "A standard reference
// frame for the description of nucleic acid base-pair geometry", J. Mol. Biol. 313, 229-237, in
// the pair's own frame: the purine (A or G) as that paper gives it, and its partner turned half a
// turn about the frame's x axis, so the pair lies flat in the plane z = 0 with its hydrogen bonds
// across the middle. Each nucleobase ends in a hydrogen where the sugar of a strand would join it, on
// the line to the sugar's C1'. Every hydrogen has its textbook bond length, in the plane, so this
// is not a force field's minimum.
//
// Atoms are numbered the purine's heavy atoms first, in the order of the paper's tables, then the
// purine's hydrogens, grouped by the atom they are on, then the partner's heavy atoms and its
// hydrogens the same way. The header's comment line lists each hydrogen bond as the atom numbers
// of its hydrogen and the atom it points to, so a scene can draw it.
package main

import (
	"fmt"
	"math"
	"os"
	"strings"
)

type vec [3]float64

func (a vec) plus(b vec) vec      { return vec{a[0] + b[0], a[1] + b[1], a[2] + b[2]} }
func (a vec) minus(b vec) vec     { return vec{a[0] - b[0], a[1] - b[1], a[2] - b[2]} }
func (a vec) times(k float64) vec { return vec{a[0] * k, a[1] * k, a[2] * k} }
func (a vec) unit() vec           { return a.times(1 / math.Sqrt(a[0]*a[0]+a[1]*a[1]+a[2]*a[2])) }

// turned is a turned by angle radians about the z axis.
func (a vec) turned(angle float64) vec {
	c, s := math.Cos(angle), math.Sin(angle)
	return vec{c*a[0] - s*a[1], s*a[0] + c*a[1], a[2]}
}

// bondToH is the length in ångströms of a bond from each element to a hydrogen.
var bondToH = map[string]float64{"C": 1.08, "N": 1.01}

type heavy struct {
	name     string
	position vec
}

// A hydrogen on an atom, which lies where the bisector of the atom's two bonds points away; for
// an amino group's two, at 120 degrees from its one bond on either side, in the plane; and for a
// methyl group's three, tetrahedral about its one bond, the first in the plane.
type hydrogen struct {
	on     string
	amino  int // 1 or -1 for the amino hydrogen turned that way from the bond
	methyl int // 1, 2 or 3 for a methyl hydrogen
	sugar  bool
}

type nucleobase struct {
	atoms     []heavy
	c1        vec // the sugar's C1', which the nucleobase's sugar hydrogen points to
	bonds     [][3]string
	hydrogens []hydrogen
}

var nucleobases = map[string]nucleobase{
	"A": {
		atoms: []heavy{
			{"N9", vec{-1.291, 4.498, 0}}, {"C8", vec{0.024, 4.897, 0}}, {"N7", vec{0.877, 3.902, 0}},
			{"C5", vec{0.071, 2.771, 0}}, {"C6", vec{0.369, 1.398, 0}}, {"N6", vec{1.611, 0.909, 0}},
			{"N1", vec{-0.668, 0.532, 0}}, {"C2", vec{-1.912, 1.023, 0}}, {"N3", vec{-2.320, 2.290, 0}},
			{"C4", vec{-1.267, 3.124, 0}},
		},
		c1: vec{-2.479, 5.346, 0},
		bonds: [][3]string{
			{"N9", "C8", "1"}, {"C8", "N7", "2"}, {"N7", "C5", "1"}, {"C5", "C6", "1"}, {"C6", "N6", "1"},
			{"C6", "N1", "2"}, {"N1", "C2", "1"}, {"C2", "N3", "2"}, {"N3", "C4", "1"}, {"C4", "C5", "2"},
			{"C4", "N9", "1"},
		},
		hydrogens: []hydrogen{{on: "N9", sugar: true}, {on: "C8"}, {on: "N6", amino: 1}, {on: "N6", amino: -1}, {on: "C2"}},
	},
	"G": {
		atoms: []heavy{
			{"N9", vec{-1.289, 4.551, 0}}, {"C8", vec{0.023, 4.962, 0}}, {"N7", vec{0.870, 3.969, 0}},
			{"C5", vec{0.071, 2.833, 0}}, {"C6", vec{0.424, 1.460, 0}}, {"O6", vec{1.554, 0.955, 0}},
			{"N1", vec{-0.700, 0.641, 0}}, {"C2", vec{-1.999, 1.087, 0}}, {"N2", vec{-2.949, 0.139, 0}},
			{"N3", vec{-2.342, 2.364, 0}}, {"C4", vec{-1.265, 3.177, 0}},
		},
		c1: vec{-2.477, 5.399, 0},
		bonds: [][3]string{
			{"N9", "C8", "1"}, {"C8", "N7", "2"}, {"N7", "C5", "1"}, {"C5", "C6", "1"}, {"C6", "O6", "2"},
			{"C6", "N1", "1"}, {"N1", "C2", "1"}, {"C2", "N2", "1"}, {"C2", "N3", "2"}, {"N3", "C4", "1"},
			{"C4", "C5", "2"}, {"C4", "N9", "1"},
		},
		hydrogens: []hydrogen{{on: "N9", sugar: true}, {on: "C8"}, {on: "N1"}, {on: "N2", amino: 1}, {on: "N2", amino: -1}},
	},
	"T": {
		atoms: []heavy{
			{"N1", vec{-1.284, 4.500, 0}}, {"C2", vec{-1.462, 3.135, 0}}, {"O2", vec{-2.562, 2.608, 0}},
			{"N3", vec{-0.298, 2.407, 0}}, {"C4", vec{0.994, 2.897, 0}}, {"O4", vec{1.944, 2.119, 0}},
			{"C5", vec{1.106, 4.338, 0}}, {"C7", vec{2.466, 4.961, 0}}, {"C6", vec{-0.024, 5.057, 0}},
		},
		c1: vec{-2.481, 5.354, 0},
		bonds: [][3]string{
			{"N1", "C2", "1"}, {"C2", "O2", "2"}, {"C2", "N3", "1"}, {"N3", "C4", "1"}, {"C4", "O4", "2"},
			{"C4", "C5", "1"}, {"C5", "C7", "1"}, {"C5", "C6", "2"}, {"C6", "N1", "1"},
		},
		hydrogens: []hydrogen{{on: "N1", sugar: true}, {on: "N3"}, {on: "C7", methyl: 1}, {on: "C7", methyl: 2}, {on: "C7", methyl: 3}, {on: "C6"}},
	},
	"C": {
		atoms: []heavy{
			{"N1", vec{-1.285, 4.542, 0}}, {"C2", vec{-1.472, 3.158, 0}}, {"O2", vec{-2.628, 2.709, 0}},
			{"N3", vec{-0.391, 2.344, 0}}, {"C4", vec{0.837, 2.868, 0}}, {"N4", vec{1.875, 2.027, 0}},
			{"C5", vec{1.056, 4.275, 0}}, {"C6", vec{-0.023, 5.068, 0}},
		},
		c1: vec{-2.477, 5.402, 0},
		bonds: [][3]string{
			{"N1", "C2", "1"}, {"C2", "O2", "2"}, {"C2", "N3", "1"}, {"N3", "C4", "2"}, {"C4", "N4", "1"},
			{"C4", "C5", "1"}, {"C5", "C6", "2"}, {"C6", "N1", "1"},
		},
		hydrogens: []hydrogen{{on: "N1", sugar: true}, {on: "N4", amino: 1}, {on: "N4", amino: -1}, {on: "C5"}, {on: "C6"}},
	},
}

// hydrogenBonds names, for each pair, each hydrogen bond's donor, the amino turn of its hydrogen
// (0 for a ring hydrogen), and its acceptor, each as nucleobase and atom.
var hydrogenBonds = map[string][][4]string{
	"AT": {{"A", "N6", "toward", "T O4"}, {"T", "N3", "", "A N1"}},
	"GC": {{"C", "N4", "toward", "G O6"}, {"G", "N1", "", "C N3"}, {"G", "N2", "toward", "C O2"}},
}

type atom struct {
	element  string
	position vec
}

type pair struct {
	atoms   []atom
	bonds   [][3]int
	numbers map[string]int // atom numbers by "A N6" for a heavy atom and "A H N6 1 0" for a hydrogen
}

func (p *pair) add(key, element string, position vec) {
	p.atoms = append(p.atoms, atom{element, position})
	p.numbers[key] = len(p.atoms)
}

// addNucleobase adds a nucleobase's atoms, with each position put through place.
func (p *pair) addNucleobase(letter string, place func(vec) vec) {
	b := nucleobases[letter]
	at := map[string]vec{}
	for _, h := range b.atoms {
		at[h.name] = place(h.position)
		p.add(letter+" "+h.name, h.name[:1], at[h.name])
	}
	neighbors := map[string][]string{}
	for _, bond := range b.bonds {
		neighbors[bond[0]] = append(neighbors[bond[0]], bond[1])
		neighbors[bond[1]] = append(neighbors[bond[1]], bond[0])
		order := map[string]int{"1": 1, "2": 2}[bond[2]]
		p.bonds = append(p.bonds, [3]int{p.numbers[letter+" "+bond[0]], p.numbers[letter+" "+bond[1]], order})
	}
	for _, h := range b.hydrogens {
		from := at[h.on]
		var way vec
		switch {
		case h.sugar:
			way = place(b.c1).minus(from).unit()
		case h.amino != 0:
			way = at[neighbors[h.on][0]].minus(from).unit().turned(float64(h.amino) * 2 * math.Pi / 3)
		case h.methyl != 0:
			bond := from.minus(at[neighbors[h.on][0]]).unit()
			angle := float64(h.methyl-1) * 2 * math.Pi / 3
			across := bond.turned(math.Pi / 2).times(math.Cos(angle)).plus(vec{0, 0, math.Sin(angle)})
			way = bond.times(1.0 / 3).plus(across.times(math.Sqrt(8) / 3))
		default:
			n := neighbors[h.on]
			way = at[n[0]].minus(from).unit().plus(at[n[1]].minus(from).unit()).times(-1).unit()
		}
		key := fmt.Sprintf("%s H %s %d %d", letter, h.on, h.amino, h.methyl)
		p.add(key, "H", from.plus(way.times(bondToH[h.on[:1]])))
		p.bonds = append(p.bonds, [3]int{p.numbers[letter+" "+h.on], len(p.atoms), 1})
	}
}

func build(name string) (pair, []string) {
	letters := strings.Split(name, "")
	p := pair{numbers: map[string]int{}}
	p.addNucleobase(letters[0], func(v vec) vec { return v })
	p.addNucleobase(letters[1], func(v vec) vec { return vec{v[0], -v[1], -v[2]} })
	var listed []string
	for _, hb := range hydrogenBonds[name] {
		donor, on, turn, acceptor := hb[0], hb[1], hb[2], hb[3]
		h := p.numbers[fmt.Sprintf("%s H %s 0 0", donor, on)]
		if turn == "toward" {
			// Of an amino group's two hydrogens, the one nearer the acceptor.
			target := p.atoms[p.numbers[acceptor]-1].position
			distance := func(n int) float64 {
				d := p.atoms[n-1].position.minus(target)
				return d[0]*d[0] + d[1]*d[1] + d[2]*d[2]
			}
			h = p.numbers[fmt.Sprintf("%s H %s 1 0", donor, on)]
			if other := p.numbers[fmt.Sprintf("%s H %s -1 0", donor, on)]; distance(other) < distance(h) {
				h = other
			}
		}
		listed = append(listed, fmt.Sprintf("%d-%d", h, p.numbers[acceptor]))
	}
	return p, listed
}

func (p pair) structureFile(name string, hydrogenBonds []string) string {
	coordinate := func(v float64) string {
		s := fmt.Sprintf("%10.4f", v)
		return strings.Replace(s, "-0.0000", " 0.0000", 1)
	}
	var b strings.Builder
	fmt.Fprintf(&b, "%s_pair\n  %-8s%10s3D\nhydrogen bonds %s\n", strings.ToLower(name), "pairs", "", strings.Join(hydrogenBonds, " "))
	fmt.Fprintf(&b, "%3d%3d  0  0  0  0  0  0  0  0999 V2000\n", len(p.atoms), len(p.bonds))
	for _, a := range p.atoms {
		fmt.Fprintf(&b, "%s%s%s %-3s 0  0  0  0  0  0  0  0  0  0  0  0\n",
			coordinate(a.position[0]), coordinate(a.position[1]), coordinate(a.position[2]), a.element)
	}
	for _, bond := range p.bonds {
		fmt.Fprintf(&b, "%3d%3d%3d  0\n", bond[0], bond[1], bond[2])
	}
	b.WriteString("M  END\n$$$$\n")
	return b.String()
}

func main() {
	if len(os.Args) != 3 || hydrogenBonds[os.Args[1]] == nil {
		fmt.Fprintln(os.Stderr, "usage: go run nucleobase_pair.go <AT|GC> <file.sdf>")
		os.Exit(2)
	}
	p, listed := build(os.Args[1])
	if err := os.WriteFile(os.Args[2], []byte(p.structureFile(os.Args[1], listed)), 0o644); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
