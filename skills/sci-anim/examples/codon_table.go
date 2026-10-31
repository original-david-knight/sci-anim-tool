//go:build ignore

// Writes codon_table.json, or the path given. Edit this, not the JSON. Run it from this folder:
//
//	go run codon_table.go
//
// The standard genetic code as a table of all 64 codons, each with the amino acid it codes for
// by its three-letter code, then the start codon AUG lit, where a ribosome begins to read. The
// code is NCBI's translation table 1, written below as NCBI writes it, so it can be checked
// against https://www.ncbi.nlm.nih.gov/Taxonomy/Utils/wprintgc.cgi letter by letter.
package main

import (
	"encoding/json"
	"fmt"
	"os"
	"reflect"
	"strconv"
	"strings"
)

// NCBI's translation table 1, The Standard Code, with T for the U of mRNA. Codon k has the bases
// base1[k], base2[k], base3[k] and codes for the amino acid aas[k], one letter each, * for stop.
const (
	aas   = "FFLLSSSSYY**CC*WLLLLPPPPHHQQRRRRIIIMTTTTNNKKSSRRVVVVAAAADDEEGGGG"
	base1 = "TTTTTTTTTTTTTTTTCCCCCCCCCCCCCCCCAAAAAAAAAAAAAAAAGGGGGGGGGGGGGGGG"
	base2 = "TTTTCCCCAAAAGGGGTTTTCCCCAAAAGGGGTTTTCCCCAAAAGGGGTTTTCCCCAAAAGGGG"
	base3 = "TCAGTCAGTCAGTCAGTCAGTCAGTCAGTCAGTCAGTCAGTCAGTCAGTCAGTCAGTCAGTCAG"
)

// Each amino acid's three-letter code, by its one-letter code.
var threeLetter = map[byte]string{
	'A': "Ala", 'R': "Arg", 'N': "Asn", 'D': "Asp", 'C': "Cys", 'Q': "Gln", 'E': "Glu",
	'G': "Gly", 'H': "His", 'I': "Ile", 'L': "Leu", 'K': "Lys", 'M': "Met", 'F': "Phe",
	'P': "Pro", 'S': "Ser", 'T': "Thr", 'W': "Trp", 'Y': "Tyr", 'V': "Val", '*': "Stop",
}

const (
	bases     = "UCAG" // the order NCBI and every textbook table lists them in
	start     = "AUG"
	gray      = "#bbbbbb"
	size      = 0.3
	padding   = 0.1
	tableX    = -1.2
	tableY    = -0.25
	captionX  = 4.8
	captionY  = -1.85 // beside AUG's row
	titleY    = 3.65
	writeTime = 4.0
)

// mRNA reads U where DNA reads T.
func rna(b byte) byte {
	if b == 'T' {
		return 'U'
	}
	return b
}

// codons is each codon of the standard code with its amino acid's three-letter code.
func codons() map[string]string {
	code := map[string]string{}
	for k := range aas {
		codon := string([]byte{rna(base1[k]), rna(base2[k]), rna(base3[k])})
		code[codon] = threeLetter[aas[k]]
	}
	if len(code) != 64 || code[start] != "Met" || code["UGG"] != "Trp" || code["UAA"] != "Stop" {
		panic("the code is not NCBI's table 1")
	}
	return code
}

// rows lays the code out as textbooks do: a column for each second base, and for each first base
// a block of four rows, one for each third base. The first column names the first base on the
// first row of its block, and the last column names each row's third base.
func rows(code map[string]string) ([]any, [2]int) {
	header := []any{"1st"}
	for i := range bases {
		header = append(header, string(bases[i]))
	}
	table := []any{append(header, "3rd")}
	var lit [2]int
	for f := range bases {
		for t := range bases {
			row := []any{""}
			if t == 0 {
				row[0] = string(bases[f])
			}
			for s := range bases {
				codon := string([]byte{bases[f], bases[s], bases[t]})
				row = append(row, codon+" "+code[codon])
				if codon == start {
					lit = [2]int{len(table), len(row) - 1}
				}
			}
			table = append(table, append(row, string(bases[t])))
		}
	}
	return table, lit
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

func main() {
	table, lit := rows(codons())
	scene := o(
		"scene_config", o("width", 1920, "height", 1080, "fps", 30, "background", "#1e1e1e"),
		"objects", o(
			"title", text("The genetic code: three bases of mRNA, one amino acid", 0.0, titleY, 0.4, "#ffffff"),
			"codons", o("type", "table", "rows", table, "size", size, "padding", padding,
				"position", []any{tableX, tableY, 0}),
			"start", text("A ribosome starts\nreading at AUG,\nwhich codes for\nmethionine (Met)", captionX, captionY, 0.3, gray),
		),
		"timeline", []obj{
			play(1.5, animation("write", "title")),
			play(writeTime, animation("write", "codons", "easing_curve", "linear")),
			o("type", "wait", "duration", 1.0),
			play(1.5,
				animation("highlight", "codons", "cell", []any{lit[0], lit[1]}, "to", 1),
				animation("write", "start")),
			o("type", "wait", "duration", 2.0),
		},
	)
	out := "codon_table.json"
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
