use std::fmt;

use aint_ast::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    Integer(i64),
    Float(f64),
    String(String),
    Identifier(String),

    // Keywords
    Let,
    Fn,
    Return,
    If,
    Else,
    True,
    False,
    Import,
    Async,
    Await,
    Infer,
    Enum,
    Tool,
    Effects,
    Test,
    Mock,
    Assert,
    Budget,
    Permissions,
    /// `as`: only meaningful after `import "path"` (milestone 29) —
    /// see `docs/milestones/29-modularity/SPEC.md`. Not reserved
    /// anywhere else in the grammar.
    As,

    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Equal,
    EqualEqual,
    BangEqual,
    Less,
    Greater,

    // Punctuation
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Comma,
    Colon,
    Arrow,

    Eof,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Integer(n) => write!(f, "integer `{n}`"),
            TokenKind::Float(n) => write!(f, "float `{n}`"),
            TokenKind::String(s) => write!(f, "string {s:?}"),
            TokenKind::Identifier(name) => write!(f, "identifier `{name}`"),
            TokenKind::Let => write!(f, "`let`"),
            TokenKind::Fn => write!(f, "`fn`"),
            TokenKind::Return => write!(f, "`return`"),
            TokenKind::If => write!(f, "`if`"),
            TokenKind::Else => write!(f, "`else`"),
            TokenKind::True => write!(f, "`true`"),
            TokenKind::False => write!(f, "`false`"),
            TokenKind::Import => write!(f, "`import`"),
            TokenKind::Async => write!(f, "`async`"),
            TokenKind::Await => write!(f, "`await`"),
            TokenKind::Infer => write!(f, "`infer`"),
            TokenKind::Enum => write!(f, "`enum`"),
            TokenKind::Tool => write!(f, "`tool`"),
            TokenKind::Effects => write!(f, "`effects`"),
            TokenKind::Test => write!(f, "`test`"),
            TokenKind::Mock => write!(f, "`mock`"),
            TokenKind::Assert => write!(f, "`assert`"),
            TokenKind::Budget => write!(f, "`budget`"),
            TokenKind::Permissions => write!(f, "`permissions`"),
            TokenKind::As => write!(f, "`as`"),
            TokenKind::Plus => write!(f, "`+`"),
            TokenKind::Minus => write!(f, "`-`"),
            TokenKind::Star => write!(f, "`*`"),
            TokenKind::Slash => write!(f, "`/`"),
            TokenKind::Equal => write!(f, "`=`"),
            TokenKind::EqualEqual => write!(f, "`==`"),
            TokenKind::BangEqual => write!(f, "`!=`"),
            TokenKind::Less => write!(f, "`<`"),
            TokenKind::Greater => write!(f, "`>`"),
            TokenKind::LeftParen => write!(f, "`(`"),
            TokenKind::RightParen => write!(f, "`)`"),
            TokenKind::LeftBrace => write!(f, "`{{`"),
            TokenKind::RightBrace => write!(f, "`}}`"),
            TokenKind::LeftBracket => write!(f, "`[`"),
            TokenKind::RightBracket => write!(f, "`]`"),
            TokenKind::Comma => write!(f, "`,`"),
            TokenKind::Colon => write!(f, "`:`"),
            TokenKind::Arrow => write!(f, "`->`"),
            TokenKind::Eof => write!(f, "end of file"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// Maps a lexed word to a keyword token, if it is one.
pub(crate) fn keyword(word: &str) -> Option<TokenKind> {
    Some(match word {
        "let" => TokenKind::Let,
        "fn" => TokenKind::Fn,
        "return" => TokenKind::Return,
        "if" => TokenKind::If,
        "else" => TokenKind::Else,
        "true" => TokenKind::True,
        "false" => TokenKind::False,
        "import" => TokenKind::Import,
        "async" => TokenKind::Async,
        "await" => TokenKind::Await,
        "infer" => TokenKind::Infer,
        "enum" => TokenKind::Enum,
        "tool" => TokenKind::Tool,
        "effects" => TokenKind::Effects,
        "test" => TokenKind::Test,
        "mock" => TokenKind::Mock,
        "assert" => TokenKind::Assert,
        "budget" => TokenKind::Budget,
        "permissions" => TokenKind::Permissions,
        "as" => TokenKind::As,
        _ => return None,
    })
}
