use std::iter::Peekable;
use std::str::Chars;

use aint_ast::{Position, Span};

use crate::error::{LexError, LexErrorKind};
use crate::token::{keyword, Token, TokenKind};

/// Scans AINT source text into tokens, one at a time.
///
/// Implements `Iterator<Item = Result<Token, LexError>>`, ending in
/// exactly one `Eof` token. Most callers want [`tokenize`] instead.
pub struct Lexer<'src> {
    chars: Peekable<Chars<'src>>,
    position: Position,
    done: bool,
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            chars: source.chars().peekable(),
            position: Position::start(),
            done: false,
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.chars.peek().copied()
    }

    fn peek_second(&self) -> Option<char> {
        let mut lookahead = self.chars.clone();
        lookahead.next();
        lookahead.next()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.chars.next()?;
        if c == '\n' {
            self.position.line += 1;
            self.position.column = 1;
        } else {
            self.position.column += 1;
        }
        Some(c)
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => {
                    self.advance();
                }
                Some('/') if self.peek_second() == Some('/') => {
                    while let Some(c) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.advance();
                    }
                }
                _ => break,
            }
        }
    }

    fn make_token(&self, kind: TokenKind, start: Position) -> Token {
        Token::new(kind, Span::new(start, self.position))
    }

    fn lex_string(&mut self, start: Position) -> Result<Token, LexError> {
        let mut value = String::new();
        loop {
            match self.peek() {
                None | Some('\n') => {
                    return Err(LexError::new(
                        LexErrorKind::UnterminatedString,
                        Span::new(start, self.position),
                    ));
                }
                Some('"') => {
                    self.advance();
                    break;
                }
                Some('\\') => {
                    self.advance();
                    match self.advance() {
                        Some('"') => value.push('"'),
                        Some('\\') => value.push('\\'),
                        Some('n') => value.push('\n'),
                        Some('t') => value.push('\t'),
                        Some('r') => value.push('\r'),
                        Some(other) => {
                            // Unrecognized escape: keep it literally
                            // rather than erroring (see SPEC.md).
                            value.push('\\');
                            value.push(other);
                        }
                        None => {
                            return Err(LexError::new(
                                LexErrorKind::UnterminatedString,
                                Span::new(start, self.position),
                            ));
                        }
                    }
                }
                Some(c) => {
                    value.push(c);
                    self.advance();
                }
            }
        }
        Ok(self.make_token(TokenKind::String(value), start))
    }

    fn lex_number(&mut self, start: Position) -> Result<Token, LexError> {
        let mut text = String::new();
        while let Some(c) = self.peek() {
            if !c.is_ascii_digit() {
                break;
            }
            text.push(c);
            self.advance();
        }

        let mut is_float = false;
        if self.peek() == Some('.') && self.peek_second().is_some_and(|c| c.is_ascii_digit()) {
            is_float = true;
            text.push('.');
            self.advance();
            while let Some(c) = self.peek() {
                if !c.is_ascii_digit() {
                    break;
                }
                text.push(c);
                self.advance();
            }
        }

        // A further '.' right after a complete number (e.g. `1.2.3`) is
        // malformed, not a valid number followed by a stray token.
        if self.peek() == Some('.') {
            while let Some(c) = self.peek() {
                if !(c.is_ascii_digit() || c == '.') {
                    break;
                }
                text.push(c);
                self.advance();
            }
            return Err(LexError::new(
                LexErrorKind::MalformedNumber(text),
                Span::new(start, self.position),
            ));
        }

        if is_float {
            let value: f64 = text.parse().map_err(|_| {
                LexError::new(
                    LexErrorKind::MalformedNumber(text.clone()),
                    Span::new(start, self.position),
                )
            })?;
            Ok(self.make_token(TokenKind::Float(value), start))
        } else {
            let value: i64 = text.parse().map_err(|_| {
                LexError::new(
                    LexErrorKind::MalformedNumber(text.clone()),
                    Span::new(start, self.position),
                )
            })?;
            Ok(self.make_token(TokenKind::Integer(value), start))
        }
    }

    fn lex_identifier(&mut self, start: Position) -> Token {
        let mut text = String::new();
        while let Some(c) = self.peek() {
            if !(c.is_alphanumeric() || c == '_') {
                break;
            }
            text.push(c);
            self.advance();
        }
        let kind = keyword(&text).unwrap_or(TokenKind::Identifier(text));
        self.make_token(kind, start)
    }

    fn lex_token(&mut self) -> Option<Result<Token, LexError>> {
        self.skip_whitespace_and_comments();

        let start = self.position;
        let c = self.peek()?;

        let result = if c.is_ascii_digit() {
            self.lex_number(start)
        } else if c.is_alphabetic() || c == '_' {
            Ok(self.lex_identifier(start))
        } else if c == '"' {
            self.advance();
            self.lex_string(start)
        } else {
            self.advance();
            self.lex_symbol(c, start)
        };

        Some(result)
    }

    fn lex_symbol(&mut self, c: char, start: Position) -> Result<Token, LexError> {
        let kind = match c {
            '+' => TokenKind::Plus,
            '-' => {
                if self.peek() == Some('>') {
                    self.advance();
                    TokenKind::Arrow
                } else {
                    TokenKind::Minus
                }
            }
            '*' => TokenKind::Star,
            '/' => TokenKind::Slash,
            '=' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::EqualEqual
                } else {
                    TokenKind::Equal
                }
            }
            '!' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::BangEqual
                } else {
                    TokenKind::Bang
                }
            }
            '<' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::LessEqual
                } else {
                    TokenKind::Less
                }
            }
            '>' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::GreaterEqual
                } else {
                    TokenKind::Greater
                }
            }
            '&' if self.peek() == Some('&') => {
                self.advance();
                TokenKind::AmpAmp
            }
            '|' if self.peek() == Some('|') => {
                self.advance();
                TokenKind::PipePipe
            }
            '(' => TokenKind::LeftParen,
            ')' => TokenKind::RightParen,
            '{' => TokenKind::LeftBrace,
            '}' => TokenKind::RightBrace,
            '[' => TokenKind::LeftBracket,
            ']' => TokenKind::RightBracket,
            ',' => TokenKind::Comma,
            ':' => TokenKind::Colon,
            _ => {
                return Err(LexError::new(
                    LexErrorKind::UnknownCharacter(c),
                    Span::new(start, self.position),
                ));
            }
        };
        Ok(self.make_token(kind, start))
    }
}

impl Iterator for Lexer<'_> {
    type Item = Result<Token, LexError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        match self.lex_token() {
            Some(result) => Some(result),
            None => {
                self.done = true;
                let eof = self.position;
                Some(Ok(Token::new(TokenKind::Eof, Span::new(eof, eof))))
            }
        }
    }
}

/// Tokenizes `source` in full, stopping at the first error.
pub fn tokenize(source: &str) -> Result<Vec<Token>, LexError> {
    Lexer::new(source).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        tokenize(src)
            .expect("should lex without errors")
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    #[test]
    fn lexes_let_binding() {
        assert_eq!(
            kinds("let x = 42"),
            vec![
                TokenKind::Let,
                TokenKind::Identifier("x".into()),
                TokenKind::Equal,
                TokenKind::Integer(42),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn keywords_are_not_identifiers() {
        assert_eq!(
            kinds("let fn return if else true false import async await"),
            vec![
                TokenKind::Let,
                TokenKind::Fn,
                TokenKind::Return,
                TokenKind::If,
                TokenKind::Else,
                TokenKind::True,
                TokenKind::False,
                TokenKind::Import,
                TokenKind::Async,
                TokenKind::Await,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn a_keyword_prefixed_identifier_is_still_an_identifier() {
        assert_eq!(
            kinds("letter"),
            vec![TokenKind::Identifier("letter".into()), TokenKind::Eof]
        );
    }

    #[test]
    fn lexes_float_literal() {
        assert_eq!(kinds("12.5"), vec![TokenKind::Float(12.5), TokenKind::Eof]);
    }

    #[test]
    fn lexes_string_literal_with_escapes() {
        assert_eq!(
            kinds("\"hello\\nworld\""),
            vec![TokenKind::String("hello\nworld".into()), TokenKind::Eof]
        );
    }

    #[test]
    fn lexes_identifier() {
        assert_eq!(
            kinds("message"),
            vec![TokenKind::Identifier("message".into()), TokenKind::Eof]
        );
    }

    #[test]
    fn lexes_each_operator() {
        assert_eq!(
            kinds("+ - * / = == != < > ! <= >= && ||"),
            vec![
                TokenKind::Plus,
                TokenKind::Minus,
                TokenKind::Star,
                TokenKind::Slash,
                TokenKind::Equal,
                TokenKind::EqualEqual,
                TokenKind::BangEqual,
                TokenKind::Less,
                TokenKind::Greater,
                TokenKind::Bang,
                TokenKind::LessEqual,
                TokenKind::GreaterEqual,
                TokenKind::AmpAmp,
                TokenKind::PipePipe,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn a_lone_ampersand_or_pipe_is_an_unknown_character() {
        // There's no bitwise AND/OR, and no single-`&`/`|` token at
        // all - only the doubled `&&`/`||` forms are real syntax.
        assert_eq!(
            tokenize("&").unwrap_err().kind,
            LexErrorKind::UnknownCharacter('&')
        );
        assert_eq!(
            tokenize("|").unwrap_err().kind,
            LexErrorKind::UnknownCharacter('|')
        );
    }

    #[test]
    fn lexes_each_punctuation_token() {
        assert_eq!(
            kinds("( ) { } [ ] , : ->"),
            vec![
                TokenKind::LeftParen,
                TokenKind::RightParen,
                TokenKind::LeftBrace,
                TokenKind::RightBrace,
                TokenKind::LeftBracket,
                TokenKind::RightBracket,
                TokenKind::Comma,
                TokenKind::Colon,
                TokenKind::Arrow,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn multi_char_operators_are_not_split() {
        assert_eq!(kinds("=="), vec![TokenKind::EqualEqual, TokenKind::Eof]);
        assert_eq!(kinds("!="), vec![TokenKind::BangEqual, TokenKind::Eof]);
        assert_eq!(kinds("->"), vec![TokenKind::Arrow, TokenKind::Eof]);
        assert_eq!(kinds("<="), vec![TokenKind::LessEqual, TokenKind::Eof]);
        assert_eq!(kinds(">="), vec![TokenKind::GreaterEqual, TokenKind::Eof]);
        assert_eq!(kinds("&&"), vec![TokenKind::AmpAmp, TokenKind::Eof]);
        assert_eq!(kinds("||"), vec![TokenKind::PipePipe, TokenKind::Eof]);
        // Not `=`, `=`, `=` or `-`, `>`.
        assert_eq!(kinds("===").len(), 3); // `==`, `=`, Eof
    }

    #[test]
    fn skips_line_comments() {
        assert_eq!(
            kinds("let x = 1 // this is a comment\nlet y = 2"),
            vec![
                TokenKind::Let,
                TokenKind::Identifier("x".into()),
                TokenKind::Equal,
                TokenKind::Integer(1),
                TokenKind::Let,
                TokenKind::Identifier("y".into()),
                TokenKind::Equal,
                TokenKind::Integer(2),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn span_after_comment_is_correct() {
        let tokens = tokenize("// comment\nlet").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Let);
        assert_eq!(tokens[0].span.start, Position::new(2, 1));
    }

    #[test]
    fn lexes_non_ascii_identifiers() {
        assert_eq!(
            kinds("café"),
            vec![TokenKind::Identifier("café".into()), TokenKind::Eof]
        );
        assert_eq!(
            kinds("π"),
            vec![TokenKind::Identifier("π".into()), TokenKind::Eof]
        );
    }

    #[test]
    fn errors_on_unterminated_string() {
        let err = tokenize("\"hello").unwrap_err();
        assert_eq!(err.kind, LexErrorKind::UnterminatedString);
    }

    #[test]
    fn errors_on_unterminated_string_at_newline() {
        let err = tokenize("\"hello\nworld\"").unwrap_err();
        assert_eq!(err.kind, LexErrorKind::UnterminatedString);
    }

    #[test]
    fn errors_on_unknown_character() {
        let err = tokenize("$").unwrap_err();
        assert_eq!(err.kind, LexErrorKind::UnknownCharacter('$'));
    }

    #[test]
    fn errors_on_malformed_number() {
        let err = tokenize("1.2.3").unwrap_err();
        assert!(matches!(err.kind, LexErrorKind::MalformedNumber(text) if text == "1.2.3"));
    }

    #[test]
    fn reports_positions_across_lines() {
        let tokens = tokenize("let\n  x").unwrap();
        let x = tokens
            .iter()
            .find(|t| matches!(t.kind, TokenKind::Identifier(_)))
            .unwrap();
        assert_eq!(x.span.start, Position::new(2, 3));
    }
}
