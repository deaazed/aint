use aint_lexer::{tokenize, TokenKind};

const HELLO_AN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/hello.an"
));

#[test]
fn tokenizes_hello_an() {
    let tokens = tokenize(HELLO_AN).expect("examples/hello.an should lex without errors");
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|t| t.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::Let,
            TokenKind::Identifier("message".into()),
            TokenKind::Equal,
            TokenKind::String("Hello, AINT!".into()),
            TokenKind::Identifier("print".into()),
            TokenKind::LeftParen,
            TokenKind::Identifier("message".into()),
            TokenKind::RightParen,
            TokenKind::Eof,
        ]
    );
}
