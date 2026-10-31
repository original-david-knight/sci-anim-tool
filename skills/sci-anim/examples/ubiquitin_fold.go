//go:build ignore

// Writes ubiquitin_fold.json, or the path given. Edit this, not the JSON. Run it from this folder:
//
//	go run ubiquitin_fold.go
//
// A chain of 76 amino acids folds into ubiquitin. The chain starts as a gentle zig-zag of 76
// curve points 3.8 Å apart, as far apart as two alpha carbons of a protein are, and morphs, curve
// point by curve point, into the atom trace through the alpha carbons of structures/1ubq.pdb.
// Both tubes are in angstroms at one scale, so the camera pulls in as the chain folds. Then the
// folded chain steps aside for the whole molecule, with its α-helix lit.
package main

import (
	"fmt"
	"os"
	"strconv"
	"strings"
)

// The chain. These are variables, not constants, because Go computes constant arithmetic
// exactly, and the committed file has float64's rounding.
var (
	residues = 76   // ubiquitin's amino acids, one curve point each
	step     = 3.5  // how far each curve point lies along x from the one before, in Å
	zig      = 0.75 // how far each lies above or below the chain's axis, in Å
)

// chain returns the zig-zag's curve points, the first on the left, centered on the origin.
// Neighbors lie sqrt(3.5² + 1.5²) = 3.81 Å apart.
func chain() []string {
	points := make([]string, residues)
	for k := range points {
		x := (float64(k) - float64(residues-1)/2) * step
		y := zig
		if k%2 == 1 {
			y = -zig
		}
		points[k] = "[" + num(x) + ", " + num(y) + ", 0]"
	}
	return points
}

func num(f float64) string {
	return strconv.FormatFloat(f, 'f', -1, 64)
}

const scene = `{
  "camera": { "position": [0, 0, 21], "target": [0, 0, 0] },
  "objects": {
    "say_chain": { "type": "text", "text": "A chain of 76 amino acids", "size": 1, "position": [0, -4.6, 0], "opacity": 0 },

    "chain": { "type": "tube", "radius": 1.2, "scale": [0.1, 0.1, 0.1],
      "curve": { "points": [
        %s
      ] },
      "material": { "type": "standard", "color": "#58c4dd" } },

    "fold": { "type": "tube", "radius": 1.2, "scale": [0.1, 0.1, 0.1],
      "curve": { "trace": { "file": "structures/1ubq.pdb", "atom_name": "CA" } },
      "material": { "type": "standard", "color": "#58c4dd" } },

    "heading": { "type": "text", "text": "The chain folds into ubiquitin", "size": 0.5, "position": [0, 3.3, 0], "opacity": 0 },

    "protein": { "type": "molecule", "file": "structures/1ubq.pdb", "scale": [0.1, 0.1, 0.1], "position": [3, -0.2, 0], "opacity": 0,
      "groups": {
        "helix": { "residues": [{ "chain": "A", "from": 23, "to": 34 }] }
      } },

    "say_helix": { "type": "text", "text": "Amino acids 23 to 34 coil into its α-helix.", "size": 0.4, "position": [0, -3.4, 0], "opacity": 0 }
  },
  "timeline": [
    { "type": "play", "duration": 2, "animations": [
      { "type": "create", "object": "chain", "easing_curve": "linear" },
      { "type": "fade", "object": "say_chain", "to": 1 }
    ] },
    { "type": "wait", "duration": 1 },
    { "type": "play", "duration": 6, "animations": [
      { "type": "morph", "object": "chain", "into": "fold" },
      { "type": "camera-move", "to": [0, 0, 10] },
      { "type": "fade", "object": "say_chain", "to": 0, "start": 0, "end": 0.2 }
    ] },
    { "type": "play", "duration": 1.5, "animations": [
      { "type": "move", "object": "fold", "to": [-3, -0.2, 0] },
      { "type": "fade", "object": "protein", "to": 1 },
      { "type": "fade", "object": "heading", "to": 1 }
    ] },
    { "type": "play", "duration": 1.5, "animations": [
      { "type": "highlight", "object": "protein", "group": "helix", "to": 1 },
      { "type": "fade", "object": "say_helix", "to": 1 }
    ] },
    { "type": "wait", "duration": 1.5 }
  ]
}
`

func main() {
	out := "ubiquitin_fold.json"
	if len(os.Args) > 1 {
		out = os.Args[1]
	}
	written := fmt.Sprintf(scene, strings.Join(chain(), ",\n        "))
	if err := os.WriteFile(out, []byte(written), 0o644); err != nil {
		panic(err)
	}
}
