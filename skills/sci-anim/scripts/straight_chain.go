//go:build ignore

// Writes the structure file of a straight polymer chain, from ideal bond lengths and angles:
//
//	go run straight_chain.go [-radical] <monomer> <length> <file.sdf>
//	go run straight_chain.go ethylene 10 decane.sdf
//
// The chain is the monomer's repeating unit over and over, with <length> backbone atoms, a ring in
// the backbone counting as one, so a chain never ends inside a ring. Every backbone dihedral is
// 180 degrees (all anti), every angle has its textbook size, and every bond its textbook length,
// so this is not a force field's minimum. The backbone's zig-zag lies in the plane z = 0 along x,
// its first atom on the left, and a ring in the backbone lies in that plane too. Each end atom's
// last bond takes a hydrogen, except the last atom's with -radical, which is left with an unpaired
// electron.
//
// Atoms are numbered backbone atoms first, left to right, a ring's six round the ring from the
// one the backbone enters by, then the side groups' heavy atoms, then the hydrogens, grouped by
// the atom they are on in the order of its number. A chain's first atoms take the same positions
// whatever its length, so a short chain is a long one cut.
package main

import (
	"flag"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"slices"
	"strconv"
	"strings"
)

// backboneAtom is one atom of a backbone, or a benzene ring the backbone enters at one ring atom
// and leaves at the one across from it, straight on, which counts as one backbone atom.
type backboneAtom struct {
	element string
	ring    bool
	bond    float64 // the length in ångströms of the bond from the backbone atom before
	angle   float64 // the bond angle at the atom in degrees, between its two backbone bonds
	// The atom's side groups. A tetrahedral atom has two, on its -z side and then its +z side, and
	// every second unit swaps them, so a chain's side groups alternate sides (syndiotactic): on one
	// side, two of them on a straight backbone would lie 2.5 Å apart and crowd each other. A
	// trigonal atom has one, in the plane of the backbone, out from its bend.
	sides []string
}

// Bond lengths in ångströms.
const (
	ringBond       = 1.39 // between two carbons of a benzene ring
	ringHydrogen   = 1.08 // from a carbon of a benzene ring to its hydrogen
	ringToCarbon   = 1.51 // from a tetrahedral carbon to the ring carbon of a phenyl
	ringToCarbonyl = 1.48 // from a ring carbon to a carbonyl carbon
	carbonylBond   = 1.21 // the double bond of C=O
	esterBond      = 1.34 // from a carbonyl carbon to its ester oxygen
	etherBond      = 1.44 // from a tetrahedral carbon to an oxygen
)

// bondToCarbon is the length in ångströms of a single bond from a tetrahedral carbon to each element.
var bondToCarbon = map[string]float64{"C": 1.54, "H": 1.09, "F": 1.35, "Cl": 1.77}

// bondToHydrogen is the length in ångströms of the bond from a backbone atom to a hydrogen.
var bondToHydrogen = map[string]float64{"C": bondToCarbon["H"], "O": 0.96}

var tetrahedral = math.Acos(-1.0 / 3)

func carbon(sides ...string) backboneAtom {
	return backboneAtom{element: "C", bond: bondToCarbon["C"], angle: tetrahedral * 180 / math.Pi, sides: sides}
}

// The monomers, each with its repeating unit's backbone, in the order the usage names them.
var monomers = []struct {
	name string
	unit []backboneAtom
}{
	{"ethylene", []backboneAtom{carbon("H", "H"), carbon("H", "H")}},
	{"propylene", []backboneAtom{carbon("H", "H"), carbon("H", "CH3")}},
	{"vinyl-chloride", []backboneAtom{carbon("H", "H"), carbon("H", "Cl")}},
	{"tetrafluoroethylene", []backboneAtom{carbon("F", "F"), carbon("F", "F")}},
	{"styrene", []backboneAtom{carbon("H", "H"), carbon("H", "phenyl")}},
	// PET's repeating unit, -O-CO-C6H4-CO-O-CH2-CH2-, from two monomers: terephthalic acid and
	// ethylene glycol.
	{"ethylene-terephthalate", []backboneAtom{
		{element: "O", bond: etherBond, angle: 117},
		{element: "C", bond: esterBond, angle: 120, sides: []string{"=O"}},
		{element: "C", ring: true, bond: ringToCarbonyl},
		{element: "C", bond: ringToCarbonyl, angle: 120, sides: []string{"=O"}},
		{element: "O", bond: esterBond, angle: 117},
		{element: "C", bond: etherBond, angle: tetrahedral * 180 / math.Pi, sides: []string{"H", "H"}},
		carbon("H", "H"),
	}},
}

type vec [3]float64

func (a vec) plus(b vec) vec      { return vec{a[0] + b[0], a[1] + b[1], a[2] + b[2]} }
func (a vec) minus(b vec) vec     { return vec{a[0] - b[0], a[1] - b[1], a[2] - b[2]} }
func (a vec) times(k float64) vec { return vec{a[0] * k, a[1] * k, a[2] * k} }
func (a vec) dot(b vec) float64   { return a[0]*b[0] + a[1]*b[1] + a[2]*b[2] }
func (a vec) unit() vec           { return a.times(1 / math.Sqrt(a.dot(a))) }
func (a vec) cross(b vec) vec {
	return vec{a[1]*b[2] - a[2]*b[1], a[2]*b[0] - a[0]*b[2], a[0]*b[1] - a[1]*b[0]}
}

var up = vec{0, 0, 1}

// backbone is where a repeating unit's backbone atoms lie: where the backbone enters each and where
// it leaves, which differ only for a ring, and how far one unit's atoms lie from the last's.
type backbone struct {
	unit         []backboneAtom
	enter, leave []vec
	repeat       vec
}

// walk lays a unit's backbone out in the plane z = 0, turning one way at one atom and the other way
// at the next, so every dihedral is anti, then turns it so the units repeat along +x and its first
// bond rises, and sets the zig-zag's middle on y = 0 and its first atom on x = 0.
func walk(unit []backboneAtom) backbone {
	n := len(unit)
	enter, leave := make([]vec, n+1), make([]vec, n+1)
	var heading float64
	turn := 1.0
	for i := 0; i <= n; i++ {
		s := unit[i%n]
		way := vec{math.Cos(heading), math.Sin(heading), 0}
		if i > 0 {
			enter[i] = leave[i-1].plus(way.times(s.bond))
		}
		leave[i] = enter[i]
		if s.ring {
			leave[i] = enter[i].plus(way.times(2 * ringBond))
			continue
		}
		heading += turn * (math.Pi - s.angle*math.Pi/180)
		turn = -turn
	}
	repeat := enter[n].minus(enter[0])
	angle := -math.Atan2(repeat[1], repeat[0])
	mirror := 1.0
	if firstBond := turned(enter[1].minus(leave[0]), angle); firstBond[1] < 0 {
		mirror = -1
	}
	var middle float64
	for i := range n {
		middle += (turned(enter[i], angle)[1] + turned(leave[i], angle)[1]) / float64(2*n)
	}
	place := func(p vec) vec {
		t := turned(p, angle)
		return vec{t[0], mirror * (t[1] - middle), 0}
	}
	b := backbone{unit: unit, repeat: place(enter[n]).minus(place(enter[0]))}
	for i := range n {
		b.enter = append(b.enter, place(enter[i]))
		b.leave = append(b.leave, place(leave[i]))
	}
	return b
}

func turned(p vec, angle float64) vec {
	c, s := math.Cos(angle), math.Sin(angle)
	return vec{p[0]*c - p[1]*s, p[0]*s + p[1]*c, p[2]}
}

// at is backbone atom i from 0, its unit and where it is entered and left. i may be -1 or the
// chain's length: the backbone goes on past both ends.
func (b backbone) at(i int) (a backboneAtom, unit int, enter, leave vec) {
	n := len(b.unit)
	unit = int(math.Floor(float64(i) / float64(n)))
	k := i - unit*n
	shift := b.repeat.times(float64(unit))
	return b.unit[k], unit, b.enter[k].plus(shift), b.leave[k].plus(shift)
}

type atom struct {
	element  string
	position vec
}

type bond struct {
	atoms [2]int // atom numbers from 1
	order int
}

type chain struct {
	atoms []atom
	bonds []bond
}

func (c *chain) add(element string, position vec, bondedTo, order int) int {
	c.atoms = append(c.atoms, atom{element, position})
	if bondedTo > 0 {
		c.bonds = append(c.bonds, bond{[2]int{bondedTo, len(c.atoms)}, order})
	}
	return len(c.atoms)
}

// ring adds a benzene ring's six carbons round from the first, which lies at center - toward
// times the bond, in the plane of toward and across, with its bonds alternately double and single.
// It gives the six atom numbers.
func (c *chain) ring(center, toward, across vec, bondedTo int) [6]int {
	var numbers [6]int
	for k := range 6 {
		a := float64(k) * math.Pi / 3
		p := center.plus(toward.times(-math.Cos(a) * ringBond)).plus(across.times(math.Sin(a) * ringBond))
		if k == 0 {
			numbers[k] = c.add("C", p, bondedTo, 1)
		} else {
			numbers[k] = c.add("C", p, numbers[k-1], 2-k%2)
		}
	}
	c.bonds = append(c.bonds, bond{[2]int{numbers[0], numbers[5]}, 2})
	return numbers
}

// hydrogen is a hydrogen to add, as the atom it is on, the way it points from it, and its bond.
type hydrogen struct {
	on     int
	way    vec
	length float64
}

// straightChain is the chain, and the number of its last backbone atom.
func straightChain(unit []backboneAtom, length int, radical bool) (chain, int) {
	b := walk(unit)
	var c chain
	// The atom numbers of each backbone atom, as the atom the backbone enters by and the one it
	// leaves by, which differ only for a ring.
	enters, leaves := make([]int, length), make([]int, length)
	var ringHydrogens []hydrogen
	for i := range length {
		s, _, enter, leave := b.at(i)
		before := 0
		if i > 0 {
			before = leaves[i-1]
		}
		if !s.ring {
			enters[i] = c.add(s.element, enter, before, 1)
			leaves[i] = enters[i]
			continue
		}
		toward := leave.minus(enter).unit()
		center := enter.plus(toward.times(ringBond))
		numbers := c.ring(center, toward, up.cross(toward), before)
		enters[i], leaves[i] = numbers[0], numbers[3]
		for _, k := range []int{1, 2, 4, 5} {
			ringHydrogens = append(ringHydrogens, hydrogen{numbers[k], c.atoms[numbers[k]-1].position.minus(center).unit(), ringHydrogen})
		}
	}
	hydrogens := ringHydrogens
	for i := range length {
		s, u, here, leave := b.at(i)
		_, _, _, previous := b.at(i - 1)
		_, _, next, _ := b.at(i + 1)
		before, after := previous.minus(here).unit(), next.minus(leave).unit()
		out := before.plus(after).times(-1).unit()
		number := enters[i]
		var sides []vec
		groups := slices.Clone(s.sides)
		switch len(groups) {
		case 1:
			sides = []vec{out}
		case 2:
			for _, z := range []float64{-1, 1} {
				sides = append(sides, out.times(math.Cos(tetrahedral/2)).plus(vec{0, 0, z * math.Sin(tetrahedral/2)}))
			}
			if u%2 != 0 {
				groups[0], groups[1] = groups[1], groups[0]
			}
		}
		for k, group := range groups {
			way := sides[k]
			switch group {
			case "H":
				hydrogens = append(hydrogens, hydrogen{number, way, bondToHydrogen[s.element]})
			case "=O":
				c.add("O", here.plus(way.times(carbonylBond)), number, 2)
			case "CH3":
				methyl := c.add("C", here.plus(way.times(bondToCarbon["C"])), number, 1)
				// The methyl's hydrogens are staggered against the backbone carbon's other bonds:
				// looking along the methyl's bond, the first lies 60 degrees round from the other
				// side group.
				other := sides[1-k]
				e1 := other.minus(way.times(other.dot(way))).unit()
				e2 := way.cross(e1)
				for _, turn := range []float64{60, 180, 300} {
					a := turn * math.Pi / 180
					h := way.times(-math.Cos(tetrahedral)).
						plus(e1.times(math.Cos(a) * math.Sin(tetrahedral))).
						plus(e2.times(math.Sin(a) * math.Sin(tetrahedral)))
					hydrogens = append(hydrogens, hydrogen{methyl, h, bondToCarbon["H"]})
				}
			case "phenyl":
				// The ring lies square to the backbone, as in polystyrene's zig-zag crystal form.
				// Turned toward the backbone, its hydrogens come within 0.7 Å of the backbone's
				// hydrogens or the next phenyl's.
				center := here.plus(way.times(ringToCarbon + ringBond))
				numbers := c.ring(center, way, way.cross(vec{1, 0, 0}).unit(), number)
				for _, n := range numbers[1:] {
					hydrogens = append(hydrogens, hydrogen{n, c.atoms[n-1].position.minus(center).unit(), ringHydrogen})
				}
			default:
				c.add(group, here.plus(way.times(bondToCarbon[group])), number, 1)
			}
		}
		if i == 0 {
			hydrogens = append(hydrogens, hydrogen{number, before, bondToHydrogen[s.element]})
		}
		if i == length-1 && !radical {
			hydrogens = append(hydrogens, hydrogen{leaves[i], after, bondToHydrogen[s.element]})
		}
	}
	slices.SortStableFunc(hydrogens, func(a, b hydrogen) int { return a.on - b.on })
	for _, h := range hydrogens {
		c.add("H", c.atoms[h.on-1].position.plus(h.way.times(h.length)), h.on, 1)
	}
	return c, leaves[length-1]
}

// structureFile is the chain as a V2000 structure file, with 3D in its header.
func (c chain) structureFile(name string, radical int) string {
	coordinate := func(v float64) string {
		s := fmt.Sprintf("%10.4f", v)
		return strings.Replace(s, "-0.0000", " 0.0000", 1)
	}
	var b strings.Builder
	fmt.Fprintf(&b, "%s\n  %-8s%10s3D\n\n", name, "straight", "")
	fmt.Fprintf(&b, "%3d%3d  0  0  0  0  0  0  0  0999 V2000\n", len(c.atoms), len(c.bonds))
	for _, a := range c.atoms {
		fmt.Fprintf(&b, "%s%s%s %-3s 0  0  0  0  0  0  0  0  0  0  0  0\n",
			coordinate(a.position[0]), coordinate(a.position[1]), coordinate(a.position[2]), a.element)
	}
	for _, bond := range c.bonds {
		fmt.Fprintf(&b, "%3d%3d%3d  0\n", bond.atoms[0], bond.atoms[1], bond.order)
	}
	if radical > 0 {
		fmt.Fprintf(&b, "M  RAD  1 %3d   2\n", radical)
	}
	b.WriteString("M  END\n$$$$\n")
	return b.String()
}

func main() {
	radical := flag.Bool("radical", false, "leave the last backbone atom with an unpaired electron, in place of its end hydrogen")
	var names []string
	for _, m := range monomers {
		names = append(names, m.name)
	}
	flag.Usage = func() {
		fmt.Fprintln(os.Stderr, "usage: go run straight_chain.go [-radical] <monomer> <length> <file.sdf>")
		fmt.Fprintln(os.Stderr, "monomers: "+strings.Join(names, ", "))
		fmt.Fprintln(os.Stderr, "length: how many backbone atoms, a ring in the backbone counting as one")
		flag.PrintDefaults()
	}
	flag.Parse()
	if flag.NArg() != 3 {
		flag.Usage()
		os.Exit(2)
	}
	i := slices.Index(names, flag.Arg(0))
	if i < 0 {
		fmt.Fprintf(os.Stderr, "straight_chain.go knows no monomer %q\n", flag.Arg(0))
		flag.Usage()
		os.Exit(2)
	}
	length, err := strconv.Atoi(flag.Arg(1))
	if err != nil || length < 1 {
		fmt.Fprintf(os.Stderr, "the length %q is not a whole number of backbone atoms from 1\n", flag.Arg(1))
		flag.Usage()
		os.Exit(2)
	}
	out := flag.Arg(2)
	c, last := straightChain(monomers[i].unit, length, *radical)
	end := 0
	if *radical {
		end = last
	}
	file := c.structureFile(strings.TrimSuffix(filepath.Base(out), ".sdf"), end)
	if err := os.WriteFile(out, []byte(file), 0o644); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
