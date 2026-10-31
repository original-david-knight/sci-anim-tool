//go:build ignore

// Writes docs/scene-reference.md, a Markdown reference of every scene-file field,
// from the JSON Schema that `sci_anim schema` prints. It reads the schema on stdin
// and writes the page on stdout.
//
// It keeps the schema's key order, which is the order src/ declares the fields in,
// so it decodes JSON into its own ordered value rather than Go maps.
//
// scripts/scene-reference builds and runs this file, and tests/docs.rs checks that
// the committed page is what it writes.
package main

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"regexp"
	"slices"
	"strings"
	"unicode"
)

type value struct {
	object bool
	fields []field
	items  []*value
	scalar any
}

type field struct {
	key string
	val *value
}

func decode(d *json.Decoder) *value {
	token, err := d.Token()
	if err != nil {
		panic(err)
	}
	delim, ok := token.(json.Delim)
	if !ok {
		return &value{scalar: token}
	}
	v := &value{object: delim == '{'}
	for d.More() {
		if v.object {
			key, err := d.Token()
			if err != nil {
				panic(err)
			}
			v.fields = append(v.fields, field{key.(string), decode(d)})
		} else {
			v.items = append(v.items, decode(d))
		}
	}
	if _, err := d.Token(); err != nil {
		panic(err)
	}
	return v
}

func (v *value) get(key string) *value {
	if v == nil {
		return nil
	}
	for _, f := range v.fields {
		if f.key == key {
			return f.val
		}
	}
	return nil
}

func (v *value) str(key string) string {
	if s, ok := v.get(key).scalarValue().(string); ok {
		return s
	}
	return ""
}

func (v *value) scalarValue() any {
	if v == nil {
		return nil
	}
	return v.scalar
}

func (v *value) strings(key string) []string {
	var out []string
	for _, item := range v.get(key).itemsOrNil() {
		out = append(out, item.scalar.(string))
	}
	return out
}

func (v *value) itemsOrNil() []*value {
	if v == nil {
		return nil
	}
	return v.items
}

func (v *value) int(key string) (int, bool) {
	n, ok := v.get(key).scalarValue().(json.Number)
	if !ok {
		return 0, false
	}
	i, err := n.Int64()
	return int(i), err == nil
}

// canonical writes v back as JSON in its own key order, leaving out object fields
// whose value is `true`, which a tagged union uses for the fields every variant shares.
func (v *value) canonical() string {
	switch {
	case v.object:
		var parts []string
		for _, f := range v.fields {
			if f.val.scalar == true {
				continue
			}
			parts = append(parts, fmt.Sprintf("%q:%s", f.key, f.val.canonical()))
		}
		return "{" + strings.Join(parts, ",") + "}"
	case v.items != nil:
		var parts []string
		for _, item := range v.items {
			parts = append(parts, item.canonical())
		}
		return "[" + strings.Join(parts, ",") + "]"
	default:
		out, err := json.Marshal(v.scalar)
		if err != nil {
			panic(err)
		}
		return string(out)
	}
}

// A def with a heading of its own rather than a heading under Shared types.
type ownHeading struct {
	def, title, linkText, noun string
}

var ownHeadings = []ownHeading{
	{"SceneConfig", "Scene config", "Scene config", ""},
	{"Camera", "Camera", "Camera", ""},
	{"StepFile", "Steps", "Step", "step"},
	{"ObjectFile", "Objects", "Object", "object"},
	{"AnimationFile", "Animations", "Animation", "animation"},
}

type doc struct {
	out  strings.Builder
	defs *value
	// Each def's link text and anchor, keyed by def name.
	links map[string][2]string
	// The heading anchor of each tagged-union variant already written, keyed by
	// its canonical JSON, so a second def with the same variants links to the first.
	written map[string]written
}

type written struct {
	anchor, sharedHeading string
}

func main() {
	d := json.NewDecoder(os.Stdin)
	d.UseNumber()
	schema := decode(d)
	if _, err := d.Token(); err != io.EOF {
		panic("stdin holds more than one JSON value")
	}
	page := build(schema)
	checkLinks(page)
	fmt.Print(page)
}

func build(schema *value) string {
	d := &doc{defs: schema.get("$defs"), links: map[string][2]string{}, written: map[string]written{}}
	for _, s := range ownHeadings {
		d.links[s.def] = [2]string{s.linkText, anchor(s.title)}
	}
	var shared []string
	for _, f := range d.defs.fields {
		if !slices.ContainsFunc(ownHeadings, func(s ownHeading) bool { return s.def == f.key }) {
			shared = append(shared, f.key)
			d.links[f.key] = [2]string{title(f.key), anchor(title(f.key))}
		}
	}
	slices.SortFunc(shared, func(a, b string) int { return strings.Compare(title(a), title(b)) })

	d.line("# Scene file reference")
	d.line("")
	d.line("A scene file is the JSON file a scene is written in: its scene config, its camera, its " +
		"objects and its timeline. This page lists every field a scene file takes. " +
		"`scripts/scene-reference` writes it from the JSON Schema that `sci_anim schema` prints, so " +
		"change a field's description in `src/`, where the schema comes from, and run the script, " +
		"rather than editing this page. [How scene files work](scene-files.md) explains how the parts fit together.")
	d.line("")

	d.line("## Contents")
	d.line("")
	d.line("- [Top level](#top-level)")
	for _, s := range ownHeadings {
		d.contentsLine(s.title, d.defs.get(s.def))
	}
	var sharedLinks []string
	for _, defName := range shared {
		sharedLinks = append(sharedLinks, d.link(defName))
	}
	d.line("- [Shared types](#shared-types): " + strings.Join(sharedLinks, ", "))
	d.line("")

	d.line("## Top level")
	d.line("")
	d.paragraph(schema.str("description"))
	d.table(schema, "", nil)

	for _, s := range ownHeadings {
		def := d.defs.get(s.def)
		d.line("## " + s.title)
		d.line("")
		d.paragraph(def.str("description"))
		if def.get("allOf") != nil {
			d.union(def, 3, s.noun, func(typeName string) string { return "`" + typeName + "`" })
		} else {
			d.table(def, "", nil)
		}
	}

	d.line("## Shared types")
	d.line("")
	d.paragraph("The types that the fields above refer to.")
	for _, defName := range shared {
		d.sharedType(defName, d.defs.get(defName))
	}
	return strings.TrimRight(d.out.String(), "\n") + "\n"
}

func (d *doc) line(s string) {
	d.out.WriteString(s + "\n")
}

// paragraph writes a description, keeping its blank-line paragraph breaks and
// joining the lines it was wrapped into.
func (d *doc) paragraph(text string) {
	if text == "" {
		return
	}
	for _, p := range strings.Split(text, "\n\n") {
		d.line(strings.ReplaceAll(p, "\n", " "))
		d.line("")
	}
}

func (d *doc) contentsLine(heading string, def *value) {
	bullet := "- [" + heading + "](#" + anchor(heading) + ")"
	var names []string
	for _, typeName := range typeNames(def) {
		names = append(names, "[`"+typeName+"`](#"+anchor(typeName)+")")
	}
	if len(names) > 0 {
		bullet += ": " + strings.Join(names, ", ")
	}
	d.line(bullet)
}

// typeNames lists the value of the field that picks each variant of a tagged union, in
// schema order, such as `circle` for an object.
func typeNames(def *value) []string {
	var names []string
	for _, branch := range def.get("allOf").itemsOrNil() {
		names = append(names, branch.get("then").get("properties").get(discriminator(def)).str("const"))
	}
	return names
}

func discriminator(def *value) string {
	return def.get("allOf").items[0].get("if").get("properties").fields[0].key
}

func (d *doc) link(def string) string {
	l, ok := d.links[def]
	if !ok {
		panic("no def named " + def)
	}
	return "[" + l[0] + "](#" + l[1] + ")"
}

// union writes a tagged union: a table of the fields every variant shares, if
// it has any besides the one that picks the variant, then one heading per variant.
func (d *doc) union(def *value, level int, noun string, variantTitle func(typeName string) string) {
	typeKey := discriminator(def)
	sharedHeading := ""
	if len(def.get("properties").fields) > 1 {
		sharedHeading = "Fields every " + noun + " takes"
		d.line(strings.Repeat("#", level) + " " + sharedHeading)
		d.line("")
		d.table(def, typeKey, nil)
	}
	for i, typeName := range typeNames(def) {
		then := def.get("allOf").items[i].get("then")
		heading := variantTitle(typeName)
		d.line(strings.Repeat("#", level) + " " + heading)
		d.line("")
		key := then.canonical()
		if earlier, ok := d.written[key]; ok {
			sentence := "The same as [`" + typeName + "`](#" + earlier.anchor + ")"
			if earlier.sharedHeading != "" {
				sentence += ", without the [" + strings.ToLower(earlier.sharedHeading[:1]) +
					earlier.sharedHeading[1:] + "](#" + anchor(earlier.sharedHeading) + ")"
			}
			d.paragraph(sentence + ".")
			continue
		}
		d.written[key] = written{anchor(heading), sharedHeading}
		d.paragraph(then.str("description"))
		d.table(then, "", &sharedFields{def, sharedHeading})
	}
}

func (d *doc) sharedType(defName string, def *value) {
	heading := title(defName)
	d.line("### " + heading)
	d.line("")
	d.paragraph(def.str("description"))
	switch {
	case def.get("allOf") != nil:
		noun := strings.ToLower(heading)
		d.union(def, 4, noun, func(typeName string) string { return "`" + typeName + "` " + noun })
	case def.get("anyOf") != nil:
		d.alternatives(def.get("anyOf").items)
	case def.get("properties") != nil:
		d.table(def, "", nil)
	case def.get("enum") != nil:
		d.paragraph("One of " + d.typeOf(def) + ".")
	default:
		d.paragraph("Written as " + d.typeOf(def) + ".")
	}
}

// alternatives lists the values a value can be. An object among them has its
// fields in a table, which a Markdown list item cannot hold, so it is a paragraph.
func (d *doc) alternatives(alts []*value) {
	d.paragraph("It is one of these.")
	inList := false
	for _, alt := range alts {
		description := strings.ReplaceAll(alt.str("description"), "\n", " ")
		if alt.get("properties") != nil {
			if inList {
				d.line("")
				inList = false
			}
			d.paragraph(strings.TrimSpace("An object with the fields below. " + description))
			d.table(alt, "", nil)
			continue
		}
		d.line(strings.TrimSpace("- " + d.typeOf(alt) + ". " + description))
		inList = true
	}
	if inList {
		d.line("")
	}
}

// The fields every variant of a tagged union takes, which a variant lists as `true`.
type sharedFields struct {
	union   *value
	heading string
}

// table writes the fields of obj but skip.
func (d *doc) table(obj *value, skip string, shared *sharedFields) {
	required := obj.strings("required")
	d.line("| Field | Type | Required | Description |")
	d.line("| --- | --- | --- | --- |")
	for _, f := range obj.get("properties").fields {
		if f.key == skip {
			continue
		}
		schema, description := f.val, f.val.str("description")
		isRequired := slices.Contains(required, f.key)
		if f.val.scalar == true {
			schema = shared.union.get("properties").get(f.key)
			description = "See [" + shared.heading + "](#" + anchor(shared.heading) + ")."
			isRequired = isRequired || slices.Contains(shared.union.strings("required"), f.key)
		}
		requiredCell := ""
		if isRequired {
			requiredCell = "yes"
		}
		d.line("| `" + f.key + "` | " + cell(d.typeOf(schema)) + " | " + requiredCell + " | " + cell(description) + " |")
	}
	d.line("")
}

func cell(s string) string {
	s = strings.Join(strings.Fields(s), " ")
	return strings.ReplaceAll(s, "|", `\|`)
}

func (d *doc) typeOf(v *value) string {
	if ref := v.str("$ref"); ref != "" {
		return d.link(strings.TrimPrefix(ref, "#/$defs/"))
	}
	if c := v.get("const"); c != nil {
		return "`" + fmt.Sprint(c.scalar) + "`"
	}
	if e := v.get("enum"); e != nil {
		var values []string
		for _, item := range e.items {
			values = append(values, "`"+fmt.Sprint(item.scalar)+"`")
		}
		return strings.Join(values, ", ")
	}
	if alts := v.get("anyOf"); alts != nil {
		var types []string
		for _, alt := range alts.items {
			types = append(types, d.typeOf(alt))
		}
		return strings.Join(types, " or ")
	}
	switch v.str("type") {
	case "array":
		return d.arrayType(v)
	case "object":
		if values := v.get("additionalProperties"); values != nil && values.object {
			key := "name"
			if values.str("$ref") == "#/$defs/ObjectFile" {
				key = "object id"
			}
			return "map of " + key + " to " + d.typeOf(values)
		}
		return "object"
	case "":
		panic("a schema with no type: " + v.canonical())
	default:
		return v.str("type")
	}
}

// arrayType writes an array of up to four items as the list of its items, such
// as `[number, number, number]`, and a longer one as `array of` its item.
func (d *doc) arrayType(v *value) string {
	if prefix := v.get("prefixItems"); prefix != nil {
		var types []string
		for _, item := range prefix.items {
			types = append(types, d.typeOf(item))
		}
		return "[" + strings.Join(types, ", ") + "]"
	}
	item := d.typeOf(v.get("items"))
	least, hasLeast := v.int("minItems")
	most, hasMost := v.int("maxItems")
	if !hasLeast || !hasMost || most > 4 {
		return "array of " + item
	}
	var lists []string
	for n := max(least, 1); n <= most; n++ {
		lists = append(lists, "["+strings.Repeat(item+", ", n-1)+item+"]")
	}
	return strings.Join(lists, " or ")
}

// title turns a def name such as `OnAtomFile` into a heading such as `On atom`.
// File is the suffix src/ gives a type that is read from a scene file.
func title(def string) string {
	bare := strings.TrimSuffix(def, "File")
	var words []string
	start := 0
	runes := []rune(bare)
	for i := 1; i < len(runes); i++ {
		prev, r := runes[i-1], runes[i]
		upperAfterLower := unicode.IsUpper(r) && !unicode.IsUpper(prev)
		upperBeforeLower := unicode.IsUpper(r) && unicode.IsUpper(prev) && i+1 < len(runes) && unicode.IsLower(runes[i+1])
		digitAfterLetter := unicode.IsDigit(r) && unicode.IsLetter(prev)
		if upperAfterLower || upperBeforeLower || digitAfterLetter {
			words = append(words, string(runes[start:i]))
			start = i
		}
	}
	words = append(words, string(runes[start:]))
	for i := 1; i < len(words); i++ {
		words[i] = strings.ToLower(words[i])
	}
	return strings.Join(words, " ")
}

// anchor is the anchor GitHub gives a heading: lowercase, spaces as hyphens,
// and every character but a letter, digit, hyphen or underscore dropped.
func anchor(heading string) string {
	var b strings.Builder
	for _, r := range strings.ToLower(heading) {
		switch {
		case r == ' ':
			b.WriteRune('-')
		case unicode.IsLetter(r) || unicode.IsDigit(r) || r == '-' || r == '_':
			b.WriteRune(r)
		}
	}
	return b.String()
}

var (
	headingLine = regexp.MustCompile(`(?m)^#+ (.+)$`)
	pageLink    = regexp.MustCompile(`\]\(#([^)]*)\)`)
)

// checkLinks panics on a link to an anchor no heading has, or on two headings
// with one anchor, which GitHub would tell apart with a suffix the links lack.
func checkLinks(page string) {
	anchors := map[string]bool{}
	for _, m := range headingLine.FindAllStringSubmatch(page, -1) {
		a := anchor(m[1])
		if anchors[a] {
			panic("two headings have the anchor #" + a)
		}
		anchors[a] = true
	}
	for _, m := range pageLink.FindAllStringSubmatch(page, -1) {
		if !anchors[m[1]] {
			panic("a link to #" + m[1] + ", which no heading has")
		}
	}
}
