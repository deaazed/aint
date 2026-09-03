//! AINT's canonical source formatter (milestone 24): `Program ->
//! String`, plus the top-level `format` entry point that parses first.
//!
//! **Known limitation, checked for and refused rather than silently
//! triggered**: `aint-lexer` discards `//` comments entirely — they
//! never become tokens, so they never reach the AST this formatter
//! prints from. Formatting a file that contains one would silently
//! delete it. Rather than do that, `format` refuses outright on any
//! file containing a `//` comment, with a clear message pointing at
//! this limitation. See `docs/milestones/24-language-tooling/SPEC.md`.

mod printer;

use std::fmt;

pub use printer::format_program;

#[derive(Debug, Clone, PartialEq)]
pub enum FormatError {
    /// The source has a `//` comment - formatting it would silently
    /// delete it, so nothing was written instead. See this crate's
    /// top doc comment.
    ContainsComments,
    Parse(String),
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatError::ContainsComments => write!(
                f,
                "this file has a `//` comment - `aint fmt` doesn't yet preserve comments \
                 through formatting (aint-lexer discards them before parsing), so nothing \
                 was written rather than silently deleting it; see \
                 docs/milestones/24-language-tooling/SPEC.md"
            ),
            FormatError::Parse(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for FormatError {}

/// Parses `source` and pretty-prints it back out. Refuses (see
/// [`FormatError::ContainsComments`]) rather than ever silently
/// dropping content.
pub fn format(source: &str) -> Result<String, FormatError> {
    if source_has_comment(source) {
        return Err(FormatError::ContainsComments);
    }
    let program =
        aint_parser::parse_source(source).map_err(|err| FormatError::Parse(err.to_string()))?;
    Ok(format_program(&program))
}

/// Whether `source` contains a `//` line comment outside of a string
/// literal - a `//` *inside* a string (a URL, say) doesn't count.
/// Mirrors `aint-lexer::lex_string`'s exact escape handling (a `\`
/// always consumes the next character, whatever it is) so this always
/// agrees with what the real lexer would consider "inside a string."
fn source_has_comment(source: &str) -> bool {
    let mut chars = source.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            match c {
                '\\' => {
                    chars.next();
                }
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '/' if chars.peek() == Some(&'/') => return true,
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_a_simple_program() {
        let output = format("let   x=1+2\nprint(x)").expect("should format");
        assert_eq!(output, "let x = 1 + 2\nprint(x)\n");
    }

    #[test]
    fn refuses_a_file_with_a_comment() {
        let err = format("let x = 1 // hello").unwrap_err();
        assert_eq!(err, FormatError::ContainsComments);
    }

    #[test]
    fn a_double_slash_inside_a_string_is_not_a_comment() {
        assert!(format("print(\"http://example.com\")").is_ok());
    }

    #[test]
    fn a_double_slash_after_an_escaped_quote_inside_a_string_is_not_a_comment() {
        assert!(format("print(\"say \\\"hi\\\" not // this\")").is_ok());
    }

    #[test]
    fn rejects_a_syntax_error_clearly() {
        let err = format("let x = ").unwrap_err();
        assert!(matches!(err, FormatError::Parse(_)));
    }

    #[test]
    fn formats_an_if_expression_on_one_line() {
        let output = format("let x=if a{1}else{2}\nprint(x)").expect("should format");
        assert_eq!(output, "let x = if a { 1 } else { 2 }\nprint(x)\n");
    }

    #[test]
    fn formats_a_chained_if_expression_flat_not_nested() {
        let output = format("let x=if a{1}else if b{2}else{3}\nprint(x)").expect("should format");
        assert_eq!(
            output,
            "let x = if a { 1 } else if b { 2 } else { 3 }\nprint(x)\n"
        );
    }

    #[test]
    fn formats_the_new_comparison_and_logical_operators() {
        let output = format("print(a<=b&&b>=a||!c)").expect("should format");
        assert_eq!(output, "print(a <= b && b >= a || !c)\n");
    }

    #[test]
    fn keeps_parens_where_and_binds_tighter_than_the_sources_or() {
        let output = format("print((a||b)&&c)").expect("should format");
        assert_eq!(output, "print((a || b) && c)\n");
    }

    #[test]
    fn formats_an_else_if_statement_chain_without_extra_indentation() {
        let output = format(concat!(
            "fn grade(n: Int) -> String {\n",
            "if n<60{return \"F\"}else if n<70{return \"D\"}else{return \"A\"}\n",
            "}"
        ))
        .expect("should format");
        assert_eq!(
            output,
            concat!(
                "fn grade(n: Int) -> String {\n",
                "    if n < 60 {\n",
                "        return \"F\"\n",
                "    } else if n < 70 {\n",
                "        return \"D\"\n",
                "    } else {\n",
                "        return \"A\"\n",
                "    }\n",
                "}\n"
            )
        );
    }
}
