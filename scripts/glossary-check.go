//go:build ignore

// Lists names in code and scene files that use a word GLOSSARY.md says to avoid.
//
// It checks names a change declares: Rust types, functions, variables, fields,
// enum variants and parameters, and JSON keys. It does not check prose, because
// prose uses common words such as "line" and "name" in their ordinary sense.
//
// By default it checks lines added on this branch since it left main, plus
// uncommitted and untracked files. Each hit needs a decision, because an avoided
// word is wrong only when it names the glossary concept.
//
// An avoided word with a note in parentheses, such as "range (an axis range is an
// axes')", has proper uses too, so only a reader can judge it. The script skips it.
//
// scripts/glossary-check builds and runs this file.
package main

import (
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"sort"
	"strconv"
	"strings"
	"unicode"
	"unicode/utf8"
)

const usage = `usage: scripts/glossary-check [--base REF | --all]

List declared names that use words GLOSSARY.md says to avoid.

  scripts/glossary-check              check this branch's added lines
  scripts/glossary-check --base REF   check lines added since REF
  scripts/glossary-check --all        check every tracked file

Exit status is 1 when there are hits, 0 when there are none.
`

const (
	spaceChars = `\t\n\v\f\r \x1c-\x1f\x{85}\p{Z}`
	wordChars  = `\p{L}\p{N}_`
)

var (
	root            string
	baseCandidates  = []string{"origin/main", "main", "origin/master", "master"}
	checkedSuffixes = map[string]bool{".rs": true, ".json": true}

	// Its leading group stands in for a word boundary before the keyword.
	keywordDeclaration = rx(`(?:^|[^\w])(?:struct|enum|trait|type|fn|mod|const|static|let(?:\s+mut)?)\s+([A-Za-z_]\w*)`)
	// A match of either that ends on the first colon of a path separator is no
	// declaration, which declaredNames checks.
	fieldDeclaration = rx(`^\s*(?:pub(?:\([\w:]+\))?\s+)?([a-z_]\w*)\s*:`)
	paramDeclaration = rx(`[({,]\s*(?:pub(?:\([\w:]+\))?\s+)?(?:mut\s+)?([a-z_]\w*)\s*:`)
	variantDecl      = rx(`^\s*([A-Z]\w*)\s*(?:[,({=]|$)`)
	jsonKey          = rx(`"([^"]+)"\s*:`)

	heading     = regexp.MustCompile(`^\*\*(.+)\*\*:$`)
	avoidEntry  = rx(`^(.+?)\s*(?:\((.*)\))?$`)
	nameBreak   = rx(`[_\-\s]+`)
	hunkStart   = regexp.MustCompile(`\+(\d+)`)
	hyphenWords = regexp.MustCompile(`^[a-z]+(?:-[a-z]+)+`)
)

// rx compiles a pattern whose \s and \w, alone or in a bracketed class, mean
// Unicode's space and word characters, as GLOSSARY.md and the checked names
// may hold any letter. Go's own \s and \w are ASCII's.
func rx(pattern string) *regexp.Regexp {
	expand := strings.NewReplacer(`\s`, spaceChars, `\w`, wordChars)
	classOrEscape := regexp.MustCompile(`\[\^?[^\]]*\]|\\[sw]`)
	pattern = classOrEscape.ReplaceAllStringFunc(pattern, func(m string) string {
		if strings.HasPrefix(m, "[") {
			return expand.Replace(m)
		}
		return "[" + expand.Replace(m) + "]"
	})
	return regexp.MustCompile(pattern)
}

func isWordRune(r rune) bool {
	return r == '_' || unicode.IsLetter(r) || unicode.IsNumber(r)
}

func isSpaceRune(r rune) bool {
	return unicode.In(r, unicode.Z) || strings.ContainsRune("\t\n\v\f\r \x1c\x1d\x1e\x1f\u0085", r)
}

// splitLines breaks text at every line boundary Unicode names, as GLOSSARY.md
// and the checked files may end lines with \r\n or \r.
func splitLines(text string) []string {
	var lines []string
	start := 0
	for i := 0; i < len(text); {
		r, size := utf8.DecodeRuneInString(text[i:])
		switch r {
		case '\n', '\r', '\v', '\f', '\x1c', '\x1d', '\x1e', '\u0085', ' ', ' ':
			lines = append(lines, text[start:i])
			if r == '\r' && strings.HasPrefix(text[i+size:], "\n") {
				size++
			}
			start = i + size
		}
		i += size
	}
	if start < len(text) {
		lines = append(lines, text[start:])
	}
	return lines
}

func splitTopLevel(text string) []string {
	var parts []string
	depth, current := 0, ""
	for _, ch := range text {
		switch ch {
		case '(':
			depth++
		case ')':
			depth--
		}
		if ch == ',' && depth == 0 {
			parts = append(parts, current)
			current = ""
		} else {
			current += string(ch)
		}
	}
	parts = append(parts, current)
	var kept []string
	for _, p := range parts {
		if p = strings.TrimFunc(p, isSpaceRune); p != "" {
			kept = append(kept, p)
		}
	}
	return kept
}

// hyphenatedPhrases finds words joined by hyphens, such as "ball-and-stick",
// that stand between word boundaries.
func hyphenatedPhrases(text string) []string {
	var phrases []string
	for i := 0; i < len(text); {
		r, size := utf8.DecodeRuneInString(text[i:])
		before, _ := utf8.DecodeLastRuneInString(text[:i])
		if r < 'a' || r > 'z' || (i > 0 && isWordRune(before)) {
			i += size
			continue
		}
		m := hyphenWords.FindStringIndex(text[i:])
		if m == nil {
			i += size
			continue
		}
		end := i + m[1]
		after, _ := utf8.DecodeRuneInString(text[end:])
		if end < len(text) && isWordRune(after) {
			// The last word runs on into a letter, so the phrase ends at the
			// hyphen before it, if it keeps two words.
			end = i + strings.LastIndexByte(text[i:end], '-')
			if !strings.Contains(text[i:end], "-") {
				i += size
				continue
			}
		}
		phrases = append(phrases, text[i:end])
		i = end
	}
	return phrases
}

type avoid struct{ word, term string }

func parseGlossary() ([]string, []avoid) {
	data, err := os.ReadFile(filepath.Join(root, "GLOSSARY.md"))
	if err != nil {
		fail(err)
	}
	text := string(data)
	// Hyphenated phrases the glossary uses, such as "ball-and-stick", are
	// domain words even when they contain an avoided word.
	var terms []string
	for _, p := range hyphenatedPhrases(text) {
		terms = append(terms, strings.ReplaceAll(p, "-", " "))
	}
	var avoids []avoid
	term := ""
	for _, line := range splitLines(text) {
		if m := heading.FindStringSubmatch(strings.TrimFunc(line, isSpaceRune)); m != nil {
			term = m[1]
			terms = append(terms, strings.ToLower(term))
		} else if rest, ok := strings.CutPrefix(line, "_Avoid_:"); ok && term != "" {
			for _, entry := range splitTopLevel(rest) {
				m := avoidEntry.FindStringSubmatch(entry)
				if m[2] == "" {
					avoids = append(avoids, avoid{strings.ToLower(strings.Trim(m[1], "`")), term})
				}
			}
		}
	}
	// An avoided word that is itself a glossary term is a cross-reference,
	// not a word to flag.
	var kept []avoid
	for _, a := range avoids {
		isTerm := false
		for _, t := range terms {
			isTerm = isTerm || t == a.word
		}
		if !isTerm {
			kept = append(kept, a)
		}
	}
	sort.SliceStable(terms, func(i, j int) bool {
		return utf8.RuneCountInString(terms[i]) > utf8.RuneCountInString(terms[j])
	})
	return terms, kept
}

// wordsOf splits a name such as `RateFunction` or `rate_fn` into its lowercase
// words, joined by spaces: an acronym, or one optional capital and its run of
// lowercase letters and digits.
func wordsOf(name string) string {
	isUpper := func(b byte) bool { return 'A' <= b && b <= 'Z' }
	isLowerOrDigit := func(b byte) bool { return 'a' <= b && b <= 'z' || '0' <= b && b <= '9' }
	var out []string
	for _, part := range nameBreak.Split(name, -1) {
		for i := 0; i < len(part); {
			upper := i
			for upper < len(part) && isUpper(part[upper]) {
				upper++
			}
			acronymEnd := upper
			if upper < len(part) && 'a' <= part[upper] && part[upper] <= 'z' {
				acronymEnd--
			}
			if acronymEnd > i {
				out = append(out, part[i:acronymEnd])
				i = acronymEnd
				continue
			}
			end := i
			if isUpper(part[end]) {
				end++
			}
			if end < len(part) && isLowerOrDigit(part[end]) {
				for end < len(part) && isLowerOrDigit(part[end]) {
					end++
				}
				out = append(out, part[i:end])
				i = end
				continue
			}
			i++
		}
	}
	return strings.ToLower(strings.Join(out, " "))
}

// phrase matches its words, apart by spaces and between word boundaries, in the
// output of wordsOf, which holds only ASCII lowercase letters, digits and spaces.
// A phrase of more than one character may also end in "s" or "es".
type phrase struct {
	words  []string
	plural bool
}

func newPhrase(text string) phrase {
	return phrase{strings.FieldsFunc(text, isSpaceRune), utf8.RuneCountInString(text) != 1}
}

// matchAt returns where a match starting at i ends, or -1.
func (p phrase) matchAt(s string, i int) int {
	isWordAt := func(j int) bool {
		return j >= 0 && j < len(s) && (s[j] == '_' || 'a' <= s[j] && s[j] <= 'z' || 'A' <= s[j] && s[j] <= 'Z' || '0' <= s[j] && s[j] <= '9')
	}
	if isWordAt(i - 1) {
		return -1
	}
	j := i
	for k, w := range p.words {
		if k > 0 {
			spaces := j
			for spaces < len(s) && s[spaces] == ' ' {
				spaces++
			}
			if spaces == j {
				return -1
			}
			j = spaces
		}
		if !strings.HasPrefix(s[j:], w) {
			return -1
		}
		j += len(w)
	}
	if p.plural {
		for _, ending := range []string{"s", "es"} {
			if strings.HasPrefix(s[j:], ending) && !isWordAt(j+len(ending)) {
				return j + len(ending)
			}
		}
	}
	if isWordAt(j) {
		return -1
	}
	return j
}

func (p phrase) replaceWithSpace(s string) string {
	var out strings.Builder
	for i := 0; i < len(s); {
		if end := p.matchAt(s, i); end > i {
			out.WriteByte(' ')
			i = end
		} else {
			out.WriteByte(s[i])
			i++
		}
	}
	return out.String()
}

func (p phrase) foundIn(s string) bool {
	for i := 0; i < len(s); i++ {
		if p.matchAt(s, i) > i {
			return true
		}
	}
	return false
}

// parameterNames keeps a match of paramDeclaration only when its colon is not
// the first of a path separator's two. No other way through the pattern from a
// rejected match's start ends on a colon, so the search goes on from the next
// character.
func parameterNames(line string) []string {
	pattern := paramDeclaration
	var names []string
	for pos := 0; pos < len(line); {
		m := pattern.FindStringSubmatchIndex(line[pos:])
		if m == nil {
			break
		}
		start, end := pos+m[0], pos+m[1]
		if strings.HasPrefix(line[end:], ":") {
			pos = start + 1
			continue
		}
		names = append(names, line[pos+m[2]:pos+m[3]])
		pos = end
	}
	return names
}

func declaredNames(path, line string) []string {
	var names []string
	if strings.HasSuffix(path, ".json") {
		for _, m := range jsonKey.FindAllStringSubmatch(line, -1) {
			names = append(names, m[1])
		}
		return names
	}
	for _, m := range keywordDeclaration.FindAllStringSubmatch(line, -1) {
		names = append(names, m[1])
	}
	if m := fieldDeclaration.FindStringSubmatchIndex(line); m != nil && !strings.HasPrefix(line[m[1]:], ":") {
		names = append(names, line[m[2]:m[3]])
	}
	names = append(names, parameterNames(line)...)
	for _, m := range variantDecl.FindAllStringSubmatch(line, -1) {
		names = append(names, m[1])
	}
	return names
}

func git(args ...string) string {
	cmd := exec.Command("git", args...)
	cmd.Dir = root
	cmd.Stderr = os.Stderr
	out, err := cmd.Output()
	if err != nil {
		fail(fmt.Errorf("git %s: %w", strings.Join(args, " "), err))
	}
	return string(out)
}

func resolveBase() string {
	for _, ref := range baseCandidates {
		cmd := exec.Command("git", "rev-parse", "--verify", "-q", ref)
		cmd.Dir = root
		if cmd.Run() == nil {
			return strings.TrimSpace(git("merge-base", ref, "HEAD"))
		}
	}
	return "HEAD"
}

type sourceLine struct {
	path   string
	number int
	text   string
}

// fileLines reads a file's lines, or none when it is missing, a directory, or
// not UTF-8.
func fileLines(path string) []sourceLine {
	data, err := os.ReadFile(filepath.Join(root, path))
	if err != nil || !utf8.Valid(data) {
		return nil
	}
	var lines []sourceLine
	for i, text := range splitLines(string(data)) {
		lines = append(lines, sourceLine{path, i + 1, text})
	}
	return lines
}

func addedLines(base string) []sourceLine {
	var lines []sourceLine
	path, number := "", 0
	diff := git("diff", "--unified=0", "--no-color", "--src-prefix=a/", "--dst-prefix=b/", base, "--")
	for _, line := range splitLines(diff) {
		switch {
		case strings.HasPrefix(line, "+++ "):
			path, _ = strings.CutPrefix(line, "+++ b/")
			if path == line {
				path = ""
			}
		case strings.HasPrefix(line, "@@"):
			number, _ = strconv.Atoi(hunkStart.FindStringSubmatch(line)[1])
		case strings.HasPrefix(line, "+") && path != "":
			lines = append(lines, sourceLine{path, number, line[1:]})
			number++
		}
	}
	for _, path := range splitLines(git("ls-files", "--others", "--exclude-standard")) {
		lines = append(lines, fileLines(path)...)
	}
	return lines
}

func fail(err error) {
	fmt.Fprintln(os.Stderr, "glossary-check:", err)
	os.Exit(2)
}

func main() {
	root = os.Getenv("SCI_ANIM_REPO")
	if root == "" {
		fail(fmt.Errorf("run scripts/glossary-check, which sets SCI_ANIM_REPO"))
	}
	flags := flag.NewFlagSet("glossary-check", flag.ExitOnError)
	flags.Usage = func() { fmt.Fprint(flags.Output(), usage) }
	base := flags.String("base", "", "git ref to diff against")
	all := flags.Bool("all", false, "check every tracked file")
	flags.Parse(os.Args[1:])
	if flags.NArg() > 0 {
		fmt.Fprintf(os.Stderr, "glossary-check: unexpected argument %q\n", flags.Arg(0))
		flags.Usage()
		os.Exit(2)
	}

	terms, avoids := parseGlossary()
	var termPhrases []phrase
	for _, t := range terms {
		termPhrases = append(termPhrases, newPhrase(t))
	}
	type rule struct {
		word       string
		phrase     phrase
		avoidedFor []string
	}
	var rules []*rule
	byWord := map[string]*rule{}
	for _, a := range avoids {
		r, ok := byWord[a.word]
		if !ok {
			r = &rule{word: a.word, phrase: newPhrase(a.word)}
			byWord[a.word] = r
			rules = append(rules, r)
		}
		r.avoidedFor = append(r.avoidedFor, a.term)
	}

	var lines []sourceLine
	if *all {
		for _, path := range splitLines(git("ls-files")) {
			if checkedSuffixes[filepath.Ext(path)] {
				lines = append(lines, fileLines(path)...)
			}
		}
	} else {
		if *base == "" {
			*base = resolveBase()
		}
		lines = addedLines(*base)
	}

	hits := 0
	for _, line := range lines {
		if !checkedSuffixes[filepath.Ext(line.path)] {
			continue
		}
		for _, name := range declaredNames(line.path, line.text) {
			words := wordsOf(name)
			for _, p := range termPhrases {
				words = p.replaceWithSpace(words)
			}
			for _, r := range rules {
				if r.phrase.foundIn(words) {
					hits++
					named := make([]string, len(r.avoidedFor))
					for i, term := range r.avoidedFor {
						named[i] = `"` + term + `"`
					}
					fmt.Printf("%s:%d: `%s` uses \"%s\", avoided for %s\n", line.path, line.number, name, r.word, strings.Join(named, ", "))
				}
			}
		}
	}
	fmt.Fprintf(os.Stderr, "%d glossary hit(s)\n", hits)
	if hits > 0 {
		os.Exit(1)
	}
}
