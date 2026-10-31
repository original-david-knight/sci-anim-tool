//! An expression: infix math in the variables its field names, such as `x^2 - 3*sin(2*x)` in a
//! function plot's `x`, or `cos(u)` in a space curve's `u`.
//!
//! An expression is a formula, not a program. It has the variables its field names, a few
//! constants and a fixed list of functions, and no assignment, loop or definition, so reading it
//! is the whole of what can go wrong: `Expression::parse` finds every unknown name and wrong
//! argument count, and the first stray lexeme, each with its column, and `eval` cannot fail. A
//! value outside a function's domain is NaN, which a plot draws as a gap.

/// An expression read and checked once, when the scene file is read.
#[derive(Clone, Debug)]
pub struct Expression {
    root: Term,
    /// The index of `t` among the variables it was read with, for a shape's expression that may
    /// read law time.
    law_time: Option<usize>,
}

/// The variables an expression's field names, in the order `Expression::eval` takes their values.
pub type Variables = &'static [&'static str];

/// A function plot's expression gives y for each `x`, and may read `t`, law time.
pub const FUNCTION_PLOT: Variables = &["x", LAW_TIME];

/// Each of a space curve's three expressions gives a coordinate for each `u`, and may read `t`.
pub const SPACE_CURVE: Variables = &["u", LAW_TIME];

/// A surface's height gives z for each point `x`, `y` of its rectangle, and may read `t`.
pub const SURFACE: Variables = &["x", "y", LAW_TIME];

/// The variable of law time. A shape's expressions name it last, so `Expression::at_law_time`
/// can fix it and leave the others where `eval` expects them.
pub const LAW_TIME: &str = "t";

/// Each of a law's three expressions gives one coordinate of its value for each `t`, law time.
pub const LAW: Variables = &["t"];

/// Each of a grid pattern's copy expressions gives a value for each copy from its index along x,
/// y and z and its place, and may read `t`.
pub const GRID_COPY: Variables = &["i", "j", "k", "x", "y", "z", LAW_TIME];

/// Each of a ring's or an along's copy expressions gives a value for each copy from its index and
/// its place, and may read `t`.
pub const COPY: Variables = &["i", "x", "y", "z", LAW_TIME];

/// Why a string is not an expression. `column` counts characters from 1.
#[derive(Debug, PartialEq)]
pub struct ExpressionError {
    pub column: usize,
    pub message: String,
}

const MAX_LENGTH: usize = 500;
const MAX_DEPTH: usize = 48;

const CONSTANTS: [(&str, f64); 3] = [
    ("pi", std::f64::consts::PI),
    ("e", std::f64::consts::E),
    ("tau", std::f64::consts::TAU),
];

enum Function {
    One(fn(f64) -> f64),
    Two(fn(f64, f64) -> f64),
}

const FUNCTIONS: [(&str, Function); 24] = [
    ("sin", Function::One(f64::sin)),
    ("cos", Function::One(f64::cos)),
    ("tan", Function::One(f64::tan)),
    ("asin", Function::One(f64::asin)),
    ("acos", Function::One(f64::acos)),
    ("atan", Function::One(f64::atan)),
    ("sinh", Function::One(f64::sinh)),
    ("cosh", Function::One(f64::cosh)),
    ("tanh", Function::One(f64::tanh)),
    ("exp", Function::One(f64::exp)),
    ("ln", Function::One(f64::ln)),
    ("log2", Function::One(f64::log2)),
    ("log10", Function::One(f64::log10)),
    ("sqrt", Function::One(f64::sqrt)),
    ("cbrt", Function::One(f64::cbrt)),
    ("abs", Function::One(f64::abs)),
    ("floor", Function::One(f64::floor)),
    ("ceil", Function::One(f64::ceil)),
    ("sign", Function::One(sign)),
    ("step", Function::One(step)),
    ("min", Function::Two(f64::min)),
    ("max", Function::Two(f64::max)),
    ("atan2", Function::Two(f64::atan2)),
    ("mod", Function::Two(modulo)),
];

/// Names agents carry over from other tools, with what to write here.
const HINTS: [(&str, &str); 8] = [
    (
        "log",
        "`log` could mean either logarithm, so write `ln` for the natural one, or `log10` or `log2`",
    ),
    ("arcsin", "write `asin`"),
    ("arccos", "write `acos`"),
    ("arctan", "write `atan`"),
    ("arctan2", "write `atan2(y, x)`"),
    ("fmod", "write `mod(a, b)`"),
    ("heaviside", "write `step(a)`"),
    ("pow", "write `a^b`"),
];

/// What every shape expression that may read law time says of `t`.
const LAW_TIME_SENTENCE: &str = " It may also read `t`, law time in seconds, so the shape changes over time while a `follow` with `\"sets\": \"shape\"` runs it, and is evaluated again at every frame time of that follow's span when the scene file is read; before that follow it is drawn at `t` of 0.";

/// Whether `name` can name a simulation's variable: a word of letters, digits and `_` that starts
/// with a letter and is not `t`, a constant or a function, which an expression would read as
/// those.
pub fn is_variable_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && name != LAW_TIME
        && !CONSTANTS.iter().any(|(constant, _)| *constant == name)
        && !FUNCTIONS.iter().any(|(function, _)| *function == name)
}

/// What a function plot's `expression` takes, for `sci_anim schema`.
pub fn function_plot_description() -> String {
    description(
        "An expression in `x`, such as `x^2 - 3*sin(2*x)`, which gives y for each x in data units. It may also read `t`, law time in seconds, as in the travelling wave `sin(2*x - 3*t)`: a `follow` with `\"sets\": \"shape\"` then runs it, and before that follow it is drawn at `t` of 0.",
        "Where the expression has no value, such as `sqrt(x)` for x below 0, the curve has a gap, and where it jumps, such as `step(x)` at 0, the curve breaks.",
    )
}

/// What one of a space curve's `x`, `y` and `z` takes, for `sci_anim schema`.
pub fn space_curve_description(coordinate: &str) -> String {
    description(
        &format!(
            "An expression in `u`, such as `cos(u)`, which gives the curve's {coordinate} in world units, from the object's position, for each u of `u_range`.{LAW_TIME_SENTENCE}"
        ),
        "It must have a finite value at every u of `u_range`: `sqrt(u)` over a range below 0 is a scene error, with the u where the value is missing.",
    )
}

/// What a surface's `z` takes, for `sci_anim schema`.
pub fn surface_description() -> String {
    description(
        &format!(
            "An expression in `x` and `y`, such as `-2*exp(-(x^2 + y^2)/2)`, which gives the sheet's height z over the point (x, y), for each x of `x_range` and y of `y_range`, as a textbook writes z = f(x, y). The height rises along the object's own up, in world units from its position.{LAW_TIME_SENTENCE}"
        ),
        "It must have a finite value within 10000 at every point of the rectangle: `sqrt(x)` over an `x_range` below 0 is a scene error, with the x and y where the value is missing.",
    )
}

/// What one of a law's `x`, `y` and `z` takes, for `sci_anim schema`.
pub fn law_description(coordinate: &str) -> String {
    description(
        &format!(
            "An expression in the one variable `t`, law time in seconds, such as `30*cos(2.21*t)`, which gives the {coordinate} of the value the follow sets: a position's {coordinate} in world units, a rotation's angle in degrees about the object's {coordinate} axis, as in `rotation`, or a scale's factor along the object's own {coordinate} axis, as in `scale`."
        ),
        "It is evaluated at every frame time of the follow's span when the scene file is read, and must have a finite value at each, and a position's must stay within 10000.",
    )
}

/// What one of a pattern copy's `position` coordinates takes, for `sci_anim schema`.
pub fn copy_position_description(coordinate: &str) -> String {
    copy_description(&format!(
        "gives the {coordinate} of the copy's position in world units, in the pattern's own space. The default is the place's {coordinate}, so `{coordinate} + 0.2*sin(i)` moves each copy off its place"
    ))
}

/// What one of a pattern copy's `rotation` angles takes, for `sci_anim schema`.
pub fn copy_rotation_description(coordinate: &str) -> String {
    copy_description(&format!(
        "gives the copy's angle in degrees about its own {coordinate} axis, turned as an object's `rotation` turns, such as `30*i`. The default is 0"
    ))
}

/// What one of a pattern copy's `scale` factors takes, for `sci_anim schema`.
pub fn copy_scale_description(coordinate: &str) -> String {
    copy_description(&format!(
        "gives the copy's factor along its own {coordinate} axis, such as `1 + 0.1*i`. A copy's scale and the pattern's size a flat copy's stroke with the rest of it. The default is 1"
    ))
}

/// What a pattern copy's `color_share` takes, for `sci_anim schema`.
pub fn color_share_description() -> String {
    copy_description(
        "gives how far the copy's colors are mixed toward `to_color`, or toward its pick of `palette`, in linear light, from 0 for the shape's own colors to 1 for that color, such as `i/9`. A value below 0 is 0, and above 1 is 1. Write it with `to_color` or `palette`. The default is 1 with a `palette`, and 0 without",
    )
}

/// What a pattern copy's `palette_index` takes as an expression, for `sci_anim schema`.
pub fn palette_index_description() -> String {
    copy_description(
        "gives which color of `palette` the copy picks, counting from 0, such as `mod(i, 4)`. Its value at each copy must be a whole number from 0 to one less than the palette's length",
    )
}

/// What a copy expression takes: `gives` says what its value is.
fn copy_description(gives: &str) -> String {
    description(
        &format!(
            "An expression in the copy's index and place, which {gives}. In a `grid` it reads `i`, `j` and `k`, the copy's index along x, y and z from 0, and in a `ring` or `along` it reads `i`, the copy's index from 0. In every layout it reads `x`, `y` and `z`, the copy's place in the pattern's own space.{LAW_TIME_SENTENCE}"
        ),
        "It is evaluated at every copy when the scene file is read, and must have a finite value at each: the first copy where it has none is a scene error, with its index and place.",
    )
}

/// What an expression field takes: `lead` says what its variables are and what it gives, and
/// `tail` what happens where it has no value. The constants and functions come from `CONSTANTS`
/// and `FUNCTIONS`, so the schema lists exactly what `parse` reads.
fn description(lead: &str, tail: &str) -> String {
    let list = |entries: Vec<&str>| {
        let entries: Vec<String> = entries.iter().map(|entry| format!("`{entry}`")).collect();
        match entries.split_last() {
            Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
            _ => entries.concat(),
        }
    };
    let constants = list(CONSTANTS.iter().map(|(name, _)| *name).collect());
    let one = list(
        FUNCTIONS
            .iter()
            .filter(|(_, f)| matches!(f, Function::One(_)))
            .map(|(name, _)| *name)
            .collect(),
    );
    let two = list(
        FUNCTIONS
            .iter()
            .filter(|(_, f)| matches!(f, Function::Two(_)))
            .map(|(name, _)| *name)
            .collect(),
    );
    format!(
        "{lead} \
         The operators are `+`, `-`, `*`, `/` and `^` for a power, with parentheses. `^` binds tightest and groups to the right, so `-a^2` is `-(a^2)`. \
         A product needs its `*`: write `2*a`, not `2a`. \
         The constants are {constants}. \
         The functions of one argument are {one}, and {two} take two, as in `max(a, 0)`. \
         `ln` is the natural logarithm, and there is no `log`. `atan2(y, x)` is the angle of the point (x, y) from the +x axis, from -pi to pi. \
         `mod(a, b)` is what is left of a after taking away whole b's, with the sign of b, so `mod(-1, 3)` is 2. \
         `step(a)` is 1 where a is 0 or more and 0 below it. Angles are in radians. \
         At most {MAX_LENGTH} characters. {tail}"
    )
}

fn sign(v: f64) -> f64 {
    if v == 0.0 || v.is_nan() {
        v
    } else {
        v.signum()
    }
}

/// 1 at 0 or more, 0 below, and NaN for NaN.
fn step(v: f64) -> f64 {
    if v >= 0.0 {
        1.0
    } else if v < 0.0 {
        0.0
    } else {
        v
    }
}

/// What is left of `a` after taking away whole `b`s, with the sign of `b`: the remainder a
/// periodic motion wants, so `mod(-1, 3)` is 2. NaN for a `b` of 0.
fn modulo(a: f64, b: f64) -> f64 {
    a - b * (a / b).floor()
}

#[derive(Clone, Debug)]
enum Term {
    Number(f64),
    /// The value at this index of the values `eval` is given.
    Variable(usize),
    Negate(Box<Term>),
    Binary(Op, Box<Term>, Box<Term>),
    One(fn(f64) -> f64, Box<Term>),
    Two(fn(f64, f64) -> f64, Box<Term>, Box<Term>),
}

#[derive(Clone, Copy, Debug)]
enum Op {
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
}

impl Expression {
    /// Reads `source`, which may name `variables`. Every unknown name and wrong argument count
    /// is reported, in column order, and then the first mistake in the expression's form, after
    /// which the rest cannot be read.
    pub fn parse(source: &str, variables: &[&str]) -> Result<Expression, Vec<ExpressionError>> {
        if source.chars().count() > MAX_LENGTH {
            return Err(vec![ExpressionError {
                column: MAX_LENGTH + 1,
                message: format!("an expression is at most {MAX_LENGTH} characters"),
            }]);
        }
        let tokens = lex(source).map_err(|e| vec![e])?;
        let mut parser = Parser {
            tokens: &tokens,
            next: 0,
            depth: 0,
            end: source.chars().count() + 1,
            variables,
            unknown: Vec::new(),
        };
        let read = parser.expression(0).and_then(|root| match parser.peek() {
            Some(token) => {
                let message = match token.lexeme {
                    Lexeme::Close => "`)` closes a parenthesis that was never opened".to_string(),
                    Lexeme::Comma => "`,` belongs between a function's arguments".to_string(),
                    _ => format!("unexpected {}", token.describe()),
                };
                Err(ExpressionError {
                    column: token.column,
                    message,
                })
            }
            None => Ok(root),
        });
        let mut errors = std::mem::take(&mut parser.unknown);
        match read {
            Ok(root) if errors.is_empty() => {
                let law_time = variables
                    .iter()
                    .position(|variable| *variable == LAW_TIME)
                    .filter(|k| *k + 1 == variables.len() && variables.len() > 1);
                Ok(Expression { root, law_time })
            }
            Ok(_) => Err(errors),
            Err(form) => {
                errors.push(form);
                Err(errors)
            }
        }
    }

    /// The expression's value at `values`, one for each of the variables it was read with, in
    /// their order. NaN where the expression has none, such as `sqrt(x)` for negative `x`, and
    /// infinite where it grows without bound, such as `1/x` at 0.
    pub fn eval(&self, values: &[f64]) -> f64 {
        self.root.eval(values)
    }

    /// Whether a shape's expression reads `t`, law time.
    pub fn reads_law_time(&self) -> bool {
        self.law_time.is_some_and(|k| self.root.reads(k))
    }

    /// The expression with `t` set to `law_time`, which `eval` then takes every other variable
    /// for, and no `t`. An expression read with no `t` among its variables is itself.
    pub fn at_law_time(&self, law_time: f64) -> Expression {
        match self.law_time {
            Some(k) => Expression {
                root: self.root.fixed(k, law_time),
                law_time: None,
            },
            None => self.clone(),
        }
    }
}

impl Term {
    fn reads(&self, k: usize) -> bool {
        match self {
            Term::Number(_) => false,
            Term::Variable(v) => *v == k,
            Term::Negate(a) | Term::One(_, a) => a.reads(k),
            Term::Binary(_, a, b) | Term::Two(_, a, b) => a.reads(k) || b.reads(k),
        }
    }

    fn fixed(&self, k: usize, value: f64) -> Term {
        let fix = |term: &Term| Box::new(term.fixed(k, value));
        match self {
            Term::Variable(v) if *v == k => Term::Number(value),
            Term::Number(_) | Term::Variable(_) => self.clone(),
            Term::Negate(a) => Term::Negate(fix(a)),
            Term::One(f, a) => Term::One(*f, fix(a)),
            Term::Binary(op, a, b) => Term::Binary(*op, fix(a), fix(b)),
            Term::Two(f, a, b) => Term::Two(*f, fix(a), fix(b)),
        }
    }

    fn eval(&self, values: &[f64]) -> f64 {
        match self {
            Term::Number(n) => *n,
            Term::Variable(k) => values[*k],
            Term::Negate(a) => -a.eval(values),
            Term::Binary(op, a, b) => {
                let (a, b) = (a.eval(values), b.eval(values));
                match op {
                    Op::Add => a + b,
                    Op::Subtract => a - b,
                    Op::Multiply => a * b,
                    Op::Divide => a / b,
                    Op::Power => a.powf(b),
                }
            }
            Term::One(f, a) => f(a.eval(values)),
            Term::Two(f, a, b) => f(a.eval(values), b.eval(values)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Lexeme {
    Number(f64),
    Word(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Open,
    Close,
    Comma,
}

#[derive(Debug, Clone)]
struct Token {
    lexeme: Lexeme,
    column: usize,
    text: String,
}

impl Token {
    fn describe(&self) -> String {
        match self.lexeme {
            Lexeme::Number(_) => format!("number `{}`", self.text),
            Lexeme::Word(_) => format!("name `{}`", self.text),
            _ => format!("`{}`", self.text),
        }
    }
}

fn lex(source: &str) -> Result<Vec<Token>, ExpressionError> {
    let chars: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let column = i + 1;
        let single = |lexeme: Lexeme| Token {
            lexeme,
            column,
            text: c.to_string(),
        };
        match c {
            c if c.is_whitespace() => {
                i += 1;
                continue;
            }
            '+' => tokens.push(single(Lexeme::Plus)),
            '-' | '\u{2212}' => tokens.push(single(Lexeme::Minus)),
            '*' | '\u{00d7}' | '\u{00b7}' => tokens.push(single(Lexeme::Star)),
            '/' | '\u{00f7}' => tokens.push(single(Lexeme::Slash)),
            '^' => tokens.push(single(Lexeme::Caret)),
            '(' => tokens.push(single(Lexeme::Open)),
            ')' => tokens.push(single(Lexeme::Close)),
            ',' => tokens.push(single(Lexeme::Comma)),
            c if c.is_ascii_digit() || c == '.' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let exponent = chars.get(i).is_some_and(|c| matches!(c, 'e' | 'E'))
                    && (chars.get(i + 1).is_some_and(char::is_ascii_digit)
                        || (chars.get(i + 1).is_some_and(|c| matches!(c, '+' | '-'))
                            && chars.get(i + 2).is_some_and(char::is_ascii_digit)));
                if exponent {
                    i += 2;
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                let text: String = chars[start..i].iter().collect();
                let value = text.parse::<f64>().map_err(|_| ExpressionError {
                    column,
                    message: format!("`{text}` is not a number"),
                })?;
                tokens.push(Token {
                    lexeme: Lexeme::Number(value),
                    column,
                    text,
                });
                continue;
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                tokens.push(Token {
                    lexeme: Lexeme::Word(text.clone()),
                    column,
                    text,
                });
                continue;
            }
            other => {
                return Err(ExpressionError {
                    column,
                    message: format!(
                        "`{other}` is not part of an expression; use digits, variables, `+ - * / ^`, parentheses and function names"
                    ),
                });
            }
        }
        i += 1;
    }
    Ok(tokens)
}

struct Parser<'a> {
    tokens: &'a [Token],
    next: usize,
    depth: usize,
    /// The column just past the last character, where "the expression ended" is reported.
    end: usize,
    variables: &'a [&'a str],
    /// Each unknown name and wrong argument count so far. Reading goes on past them, as a NaN.
    unknown: Vec<ExpressionError>,
}

/// Binding powers: `+ -` bind loosest, then `* /`, then unary minus, then `^`, which groups to
/// the right. So `-x^2` is `-(x^2)`, `2^-x` is `2^(-x)` and `2^3^2` is `2^(3^2)`.
const NEGATE_POWER: u8 = 5;

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.next)
    }

    fn expression(&mut self, min_power: u8) -> Result<Term, ExpressionError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            let column = self.peek().map_or(self.end, |t| t.column);
            return Err(ExpressionError {
                column,
                message: format!("parentheses and operators nest more than {MAX_DEPTH} deep"),
            });
        }
        let mut left = self.operand()?;
        while let Some(token) = self.peek() {
            let (op, left_power, right_power) = match token.lexeme {
                Lexeme::Plus => (Op::Add, 1, 2),
                Lexeme::Minus => (Op::Subtract, 1, 2),
                Lexeme::Star => (Op::Multiply, 3, 4),
                Lexeme::Slash => (Op::Divide, 3, 4),
                Lexeme::Caret => (Op::Power, 7, 6),
                Lexeme::Number(_) | Lexeme::Word(_) | Lexeme::Open => {
                    let written = if matches!(token.lexeme, Lexeme::Open) {
                        "a parenthesis".to_string()
                    } else {
                        token.describe()
                    };
                    return Err(ExpressionError {
                        column: token.column,
                        message: format!(
                            "{written} follows a value with no operator between them; write `*` for a product, as in `2*x`"
                        ),
                    });
                }
                _ => break,
            };
            if left_power < min_power {
                break;
            }
            self.next += 1;
            let right = self.expression(right_power)?;
            left = Term::Binary(op, Box::new(left), Box::new(right));
        }
        self.depth -= 1;
        Ok(left)
    }

    fn operand(&mut self) -> Result<Term, ExpressionError> {
        let Some(token) = self.tokens.get(self.next).cloned() else {
            let after = self
                .tokens
                .last()
                .map_or("the start".to_string(), |t| format!("`{}`", t.text));
            return Err(ExpressionError {
                column: self.end,
                message: format!("the expression ends after {after}, where a value is needed"),
            });
        };
        self.next += 1;
        match token.lexeme {
            Lexeme::Number(n) => Ok(Term::Number(n)),
            Lexeme::Minus => Ok(Term::Negate(Box::new(self.expression(NEGATE_POWER)?))),
            Lexeme::Plus => self.expression(NEGATE_POWER),
            Lexeme::Open => {
                let inner = self.expression(0)?;
                match self.tokens.get(self.next) {
                    Some(Token {
                        lexeme: Lexeme::Close,
                        ..
                    }) => {
                        self.next += 1;
                        Ok(inner)
                    }
                    Some(other) => Err(ExpressionError {
                        column: other.column,
                        message: format!(
                            "expected `)` to close the parenthesis at column {}, found {}",
                            token.column,
                            other.describe()
                        ),
                    }),
                    None => Err(ExpressionError {
                        column: self.end,
                        message: format!(
                            "the parenthesis at column {} is never closed",
                            token.column
                        ),
                    }),
                }
            }
            Lexeme::Word(ref name) => self.word(name, &token),
            _ => Err(ExpressionError {
                column: token.column,
                message: format!("{} cannot start a value", token.describe()),
            }),
        }
    }

    fn word(&mut self, name: &str, token: &Token) -> Result<Term, ExpressionError> {
        if matches!(
            self.peek(),
            Some(Token {
                lexeme: Lexeme::Open,
                ..
            })
        ) {
            return self.call(name, token);
        }
        if let Some(k) = self.variables.iter().position(|variable| *variable == name) {
            return Ok(Term::Variable(k));
        }
        if let Some((_, value)) = CONSTANTS.iter().find(|(constant, _)| *constant == name) {
            return Ok(Term::Number(*value));
        }
        let named: Vec<String> = self
            .variables
            .iter()
            .map(|variable| format!("`{variable}`"))
            .collect();
        let variables = match named.as_slice() {
            [one] => format!("the only variable is {one}"),
            [rest @ .., last] => format!("the variables are {} and {last}", rest.join(", ")),
            [] => "there are no variables".to_string(),
        };
        let mut message =
            format!("unknown name `{name}`; {variables}, and the constants are pi, e and tau");
        if let Some(close) = suggest(
            name,
            self.variables.iter().copied().chain(["pi", "e", "tau"]),
        ) {
            message.push_str(&format!("; did you mean `{close}`?"));
        }
        if FUNCTIONS.iter().any(|(function, _)| *function == name) {
            message = format!(
                "`{name}` is a function, so it needs parentheses, as in `{name}({})`",
                self.variables[0]
            );
        }
        self.unknown.push(ExpressionError {
            column: token.column,
            message,
        });
        Ok(Term::Number(f64::NAN))
    }

    fn call(&mut self, name: &str, token: &Token) -> Result<Term, ExpressionError> {
        let function = FUNCTIONS
            .iter()
            .find(|(function, _)| *function == name)
            .map(|(_, function)| function);
        if function.is_none() {
            let mut message = format!("unknown function `{name}`");
            if let Some((_, hint)) = HINTS.iter().find(|(hinted, _)| *hinted == name) {
                message.push_str(&format!("; {hint}"));
            } else if let Some(close) =
                suggest(name, FUNCTIONS.iter().map(|(function, _)| *function))
            {
                message.push_str(&format!("; did you mean `{close}`?"));
            }
            let names: Vec<&str> = FUNCTIONS.iter().map(|(function, _)| *function).collect();
            message.push_str(&format!("; the functions are {}", names.join(", ")));
            self.unknown.push(ExpressionError {
                column: token.column,
                message,
            });
        }
        let open = self.next;
        self.next += 1;
        let mut arguments = Vec::new();
        if matches!(
            self.peek(),
            Some(Token {
                lexeme: Lexeme::Close,
                ..
            })
        ) {
            self.next += 1;
        } else {
            loop {
                arguments.push(self.expression(0)?);
                match self.tokens.get(self.next) {
                    Some(Token {
                        lexeme: Lexeme::Comma,
                        ..
                    }) => self.next += 1,
                    Some(Token {
                        lexeme: Lexeme::Close,
                        ..
                    }) => {
                        self.next += 1;
                        break;
                    }
                    Some(other) => {
                        return Err(ExpressionError {
                            column: other.column,
                            message: format!(
                                "expected `,` or `)` in the arguments of `{name}`, found {}",
                                other.describe()
                            ),
                        });
                    }
                    None => {
                        return Err(ExpressionError {
                            column: self.end,
                            message: format!(
                                "the parenthesis at column {} is never closed",
                                self.tokens[open].column
                            ),
                        });
                    }
                }
            }
        }
        let Some(function) = function else {
            return Ok(Term::Number(f64::NAN));
        };
        let takes = match function {
            Function::One(_) => 1,
            Function::Two(_) => 2,
        };
        if arguments.len() != takes {
            let plural = if takes == 1 { "" } else { "s" };
            self.unknown.push(ExpressionError {
                column: token.column,
                message: format!(
                    "`{name}` takes {takes} argument{plural}, and this gives {}",
                    arguments.len()
                ),
            });
            return Ok(Term::Number(f64::NAN));
        }
        let mut arguments = arguments.into_iter();
        let mut next = || Box::new(arguments.next().expect("the count was checked"));
        Ok(match function {
            Function::One(f) => Term::One(*f, next()),
            Function::Two(f) => Term::Two(*f, next(), next()),
        })
    }
}

/// The candidate within a small edit distance of `name`, ignoring case.
fn suggest<'a>(name: &str, candidates: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let allowed = if name.chars().count() >= 5 { 2 } else { 1 };
    let lower = name.to_lowercase();
    let candidates: Vec<&str> = candidates.collect();
    if let Some(same) = candidates.iter().find(|c| **c == lower) {
        return Some(same);
    }
    if name.chars().count() < 3 {
        return None;
    }
    candidates
        .into_iter()
        .map(|c| (edit_distance(&lower, c), c))
        .filter(|(d, _)| *d <= allowed)
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c)
}

fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (diagonal + usize::from(ca != cb))
                .min(row[j] + 1)
                .min(above + 1);
            diagonal = above;
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(source: &str, x: f64) -> f64 {
        Expression::parse(source, FUNCTION_PLOT).unwrap().eval(&[x])
    }

    fn errors(source: &str, variables: Variables) -> Vec<(usize, String)> {
        Expression::parse(source, variables)
            .unwrap_err()
            .into_iter()
            .map(|e| (e.column, e.message))
            .collect()
    }

    fn error(source: &str) -> (usize, String) {
        let [e] = <[_; 1]>::try_from(errors(source, FUNCTION_PLOT)).expect("one error");
        e
    }

    #[test]
    fn precedence_and_associativity_follow_math() {
        assert_eq!(value("1 + 2 * 3", 0.0), 7.0);
        assert_eq!(value("-x^2", 3.0), -9.0, "minus binds looser than ^");
        assert_eq!(value("2^3^2", 0.0), 512.0, "^ groups to the right");
        assert_eq!(value("2^-x", 2.0), 0.25);
        assert_eq!(value("(1 + 2) * x", 4.0), 12.0);
        assert_eq!(value("10 - 4 - 3", 0.0), 3.0, "- groups to the left");
        assert_eq!(value("8 / 4 / 2", 0.0), 1.0);
        assert_eq!(value("x^2 - 3*x + 2", 3.0), 2.0);
        assert_eq!(value("1.5e2 + .5", 0.0), 150.5);
    }

    #[test]
    fn functions_and_constants() {
        assert!((value("sin(pi/2)", 0.0) - 1.0).abs() < 1e-12);
        assert_eq!(value("max(x, 2) + min(x, 2)", 5.0), 7.0);
        assert_eq!(value("sqrt(16) + abs(-3) + floor(2.7)", 0.0), 9.0);
        assert!((value("ln(e^2)", 0.0) - 2.0).abs() < 1e-12);
        assert!(
            value("sqrt(x)", -1.0).is_nan(),
            "outside the domain is NaN, which a plot draws as a gap"
        );
        assert_eq!(value("1/x", 0.0), f64::INFINITY);
    }

    #[test]
    fn each_mistake_is_reported_at_its_column_with_the_fix() {
        assert_eq!(
            error("x^"),
            (
                3,
                "the expression ends after `^`, where a value is needed".into()
            )
        );
        assert_eq!(error("2x"), (2, "name `x` follows a value with no operator between them; write `*` for a product, as in `2*x`".into()));
        assert_eq!(error("3 (x)"), (3, "a parenthesis follows a value with no operator between them; write `*` for a product, as in `2*x`".into()));
        assert_eq!(
            error("(x + 1"),
            (7, "the parenthesis at column 1 is never closed".into())
        );
        assert_eq!(
            error("x + 1)"),
            (6, "`)` closes a parenthesis that was never opened".into())
        );
        assert_eq!(error("x # 2"), (3, "`#` is not part of an expression; use digits, variables, `+ - * / ^`, parentheses and function names".into()));
        assert_eq!(
            error("y + 1").1,
            "unknown name `y`; the variables are `x` and `t`, and the constants are pi, e and tau"
        );
        assert_eq!(
            error("X").1,
            "unknown name `X`; the variables are `x` and `t`, and the constants are pi, e and tau; did you mean `x`?"
        );
        assert_eq!(
            error("sin").1,
            "`sin` is a function, so it needs parentheses, as in `sin(x)`"
        );
        assert_eq!(
            error("sin(x, 2)"),
            (1, "`sin` takes 1 argument, and this gives 2".into())
        );
        assert_eq!(
            error("max(x)"),
            (1, "`max` takes 2 arguments, and this gives 1".into())
        );
        assert_eq!(error("1.2.3 + x"), (1, "`1.2.3` is not a number".into()));
        assert_eq!(
            error(""),
            (
                1,
                "the expression ends after the start, where a value is needed".into()
            )
        );
    }

    #[test]
    fn an_unknown_function_names_the_fix_or_the_list() {
        let (column, message) = error("2 + sine(x)");
        assert_eq!(column, 5);
        assert!(
            message.starts_with(
                "unknown function `sine`; did you mean `sin`?; the functions are sin, cos, tan"
            ),
            "{message}"
        );
        assert!(
            error("log(x)")
                .1
                .starts_with("unknown function `log`; `log` could mean either logarithm")
        );
        assert!(error("arcsin(x)").1.contains("write `asin`"));
    }

    #[test]
    fn atan2_mod_and_step() {
        assert!(
            (value("atan2(1, -1)", 0.0) - 3.0 * std::f64::consts::FRAC_PI_4).abs() < 1e-12,
            "y first, as in math"
        );
        assert!((value("atan2(-1, 0)", 0.0) + std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert_eq!(value("mod(x, 3)", 7.0), 1.0);
        assert_eq!(
            value("mod(x, 3)", -1.0),
            2.0,
            "the sign of b, so a periodic motion wraps the same way below 0"
        );
        assert_eq!(value("mod(x, -3)", 1.0), -2.0);
        assert!(value("mod(x, 0)", 1.0).is_nan());
        assert_eq!(
            (
                value("step(x)", 0.0),
                value("step(x)", 2.0),
                value("step(x)", -0.5)
            ),
            (1.0, 1.0, 0.0)
        );
        assert!(value("step(sqrt(x))", -1.0).is_nan());
    }

    #[test]
    fn variables_are_the_ones_the_field_names() {
        let curve = Expression::parse("cos(u) + 2*u", SPACE_CURVE).unwrap();
        assert_eq!(curve.eval(&[0.0]), 1.0);
        assert_eq!(errors("x + 1", SPACE_CURVE), vec![(1, "unknown name `x`; the variables are `u` and `t`, and the constants are pi, e and tau".into())]);
        assert_eq!(errors("u", FUNCTION_PLOT), vec![(1, "unknown name `u`; the variables are `x` and `t`, and the constants are pi, e and tau".into())]);
        const COPY_INDEX: Variables = &["i", "j", "k"];
        let indexed = Expression::parse("i + 10*j + 100*k", COPY_INDEX).unwrap();
        assert_eq!(indexed.eval(&[1.0, 2.0, 3.0]), 321.0);
        assert!(
            errors("t", COPY_INDEX)[0]
                .1
                .contains("the variables are `i`, `j` and `k`")
        );
    }

    #[test]
    fn a_shape_expression_reads_law_time_once_it_is_fixed() {
        let wave = Expression::parse("sin(2*x - 3*t)", FUNCTION_PLOT).unwrap();
        let unmoving = Expression::parse("sin(2*x)", FUNCTION_PLOT).unwrap();

        assert!(wave.reads_law_time());
        assert!(!unmoving.reads_law_time());
        assert_eq!(wave.at_law_time(1.0).eval(&[1.5]), 0.0);
        assert_eq!(wave.at_law_time(1.0).eval(&[2.0]), (1.0f64).sin());
        assert!(
            !Expression::parse("t", LAW).unwrap().reads_law_time(),
            "a law's `t` is its only variable, not a shape's law time"
        );
    }

    #[test]
    fn every_unknown_name_is_reported_at_its_column_then_the_first_mistake_of_form() {
        assert_eq!(
            errors("s*cos(s) + sine(u) + max(u)", SPACE_CURVE),
            vec![
                (1, "unknown name `s`; the variables are `u` and `t`, and the constants are pi, e and tau".into()),
                (7, "unknown name `s`; the variables are `u` and `t`, and the constants are pi, e and tau".into()),
                (12, "unknown function `sine`; did you mean `sin`?; the functions are sin, cos, tan, asin, acos, atan, sinh, cosh, tanh, exp, ln, log2, log10, sqrt, cbrt, abs, floor, ceil, sign, step, min, max, atan2, mod".into()),
                (22, "`max` takes 2 arguments, and this gives 1".into()),
            ]
        );
        assert_eq!(errors("y + (x", FUNCTION_PLOT), vec![(1, "unknown name `y`; the variables are `x` and `t`, and the constants are pi, e and tau".into()), (7, "the parenthesis at column 5 is never closed".into())]);
        assert!(
            errors("arctan2(1, u)", SPACE_CURVE)[0]
                .1
                .contains("write `atan2(y, x)`")
        );
    }

    #[test]
    fn nesting_and_length_are_bounded() {
        let deep = format!("{}x{}", "(".repeat(100), ")".repeat(100));
        assert!(error(&deep).1.contains("nest more than 48 deep"));
        assert!(
            error(&"x+".repeat(300))
                .1
                .contains("at most 500 characters")
        );
    }
}
