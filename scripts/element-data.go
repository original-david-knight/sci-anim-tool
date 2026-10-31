//go:build ignore

// Extracts element colors, van der Waals radii and covalent radii from their sources.
//
// Writes tests/fixtures/elements/jmol_colors.tsv, bodr_radii.tsv and
// bodr_covalent_radii.tsv, which src/element.rs is tested against. With --rust
// it also prints the rows of the table in src/element.rs.
// tests/fixtures/elements/NOTICE says where to get the two source files.
//
// scripts/element-data builds and runs this file.
package main

import (
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
)

const usage = "usage: scripts/element-data JMOL_CONSTANTS_JS BODR_ELEMENTS_XML [--rust]\n"

var (
	jmolRow        = regexp.MustCompile(`elemParams\[(\d+)\]\s*=\s*\['(\w+)','([0-9A-Fa-f]{6})'`)
	bodrAtom       = regexp.MustCompile(`(?s)<atom id="(\w+)">(.*?)</atom>`)
	atomicNumber   = regexp.MustCompile(`atomicNumber"[^>]*>(\d+)<`)
	radiusVDW      = regexp.MustCompile(`radiusVDW"[^>]*>([\d.]+)<`)
	radiusCovalent = regexp.MustCompile(`radiusCovalent"[^>]*>([\d.]+)<`)
)

type color struct {
	number      int
	symbol, hex string
}

type radius struct{ symbol, angstroms, covalent string }

// latin1 decodes Jmol's file, whose comments are Latin-1, byte for byte.
func latin1(data []byte) string {
	runes := make([]rune, len(data))
	for i, b := range data {
		runes[i] = rune(b)
	}
	return string(runes)
}

// floatLiteral writes a radius as the table in src/element.rs has it: the
// shortest digits that read back as the same float, with ".0" on a whole
// number, and an exponent below 1e-4 or from 1e16 up.
func floatLiteral(f float64) string {
	exponent, _ := strconv.Atoi(strings.SplitN(strconv.FormatFloat(f, 'e', -1, 64), "e", 2)[1])
	if exponent < -4 || exponent >= 16 {
		return strconv.FormatFloat(f, 'e', -1, 64)
	}
	s := strconv.FormatFloat(f, 'f', -1, 64)
	if !strings.Contains(s, ".") {
		s += ".0"
	}
	return s
}

func fail(format string, args ...any) {
	fmt.Fprintf(os.Stderr, "element-data: "+format+"\n", args...)
	os.Exit(1)
}

func main() {
	repo := os.Getenv("SCI_ANIM_REPO")
	if repo == "" {
		fail("run scripts/element-data, which sets SCI_ANIM_REPO")
	}
	var files []string
	rust := false
	for _, arg := range os.Args[1:] {
		switch {
		case arg == "--rust":
			rust = true
		case arg == "-h" || arg == "--help":
			fmt.Print(usage)
			return
		case strings.HasPrefix(arg, "-"):
			fail("unknown flag %s\n%s", arg, usage)
		default:
			files = append(files, arg)
		}
	}
	if len(files) != 2 {
		fail("needs two files\n%s", usage)
	}

	jmolData, err := os.ReadFile(files[0])
	if err != nil {
		fail("%v", err)
	}
	var colors []color
	for _, m := range jmolRow.FindAllStringSubmatch(latin1(jmolData), -1) {
		n, _ := strconv.Atoi(m[1])
		colors = append(colors, color{n, m[2], strings.ToUpper(m[3])})
	}

	bodrData, err := os.ReadFile(files[1])
	if err != nil {
		fail("%v", err)
	}
	radii := map[int]radius{}
	for _, m := range bodrAtom.FindAllStringSubmatch(string(bodrData), -1) {
		number := atomicNumber.FindStringSubmatch(m[2])
		if number == nil {
			fail("BODR's atom %s has no atomic number", m[1])
		}
		n, _ := strconv.Atoi(number[1])
		r := radiusVDW.FindStringSubmatch(m[2])
		c := radiusCovalent.FindStringSubmatch(m[2])
		if 1 <= n && n <= len(colors) && r != nil && c != nil {
			f, errVDW := strconv.ParseFloat(r[1], 64)
			g, errCovalent := strconv.ParseFloat(c[1], 64)
			if errVDW != nil || errCovalent != nil {
				fail("BODR's atom %s has radii %q and %q", m[1], r[1], c[1])
			}
			if f > 0 && g > 0 {
				radii[n] = radius{m[1], r[1], c[1]}
			}
		}
	}

	var missing, mismatched []string
	for i, c := range colors {
		if c.number != i+1 {
			fail("Jmol's rows are not 1 to N in order")
		}
		r, ok := radii[c.number]
		if !ok {
			missing = append(missing, c.symbol)
		} else if r.symbol != c.symbol {
			mismatched = append(mismatched, fmt.Sprintf("%d %s %s", c.number, c.symbol, r.symbol))
		}
	}
	if len(missing) > 0 {
		fail("no BODR radii for %v", missing)
	}
	if len(mismatched) > 0 {
		fail("Jmol and BODR disagree on symbols: %v", mismatched)
	}

	out := filepath.Join(repo, "tests", "fixtures", "elements")
	if err := os.MkdirAll(out, 0o755); err != nil {
		fail("%v", err)
	}
	var colorRows, radiusRows, covalentRows strings.Builder
	var rowCalls []string
	for _, c := range colors {
		r := radii[c.number]
		fmt.Fprintf(&colorRows, "%d\t%s\t%s\n", c.number, c.symbol, c.hex)
		fmt.Fprintf(&radiusRows, "%d\t%s\t%s\n", c.number, r.symbol, r.angstroms)
		fmt.Fprintf(&covalentRows, "%d\t%s\t%s\n", c.number, r.symbol, r.covalent)
		f, _ := strconv.ParseFloat(r.angstroms, 64)
		g, _ := strconv.ParseFloat(r.covalent, 64)
		rowCalls = append(rowCalls, fmt.Sprintf(`row("%s", 0x%s, %s, %s)`, c.symbol, strings.ToLower(c.hex), floatLiteral(f), floatLiteral(g)))
	}
	for name, rows := range map[string]string{"jmol_colors.tsv": colorRows.String(), "bodr_radii.tsv": radiusRows.String(), "bodr_covalent_radii.tsv": covalentRows.String()} {
		if err := os.WriteFile(filepath.Join(out, name), []byte(rows), 0o644); err != nil {
			fail("%v", err)
		}
	}
	if rust {
		for i := 0; i < len(rowCalls); i += 4 {
			fmt.Println("    " + strings.Join(rowCalls[i:min(i+4, len(rowCalls))], ", ") + ",")
		}
	}
}
