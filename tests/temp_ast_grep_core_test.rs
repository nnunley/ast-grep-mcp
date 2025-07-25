use anyhow::Result;
use ast_grep_core::{AstGrep, Pattern};
use ast_grep_language::SupportLang as Language;
use std::str::FromStr;
#[tokio::test]
async fn test_ast_grep_core_pattern_direct() -> Result<()> {
    let code = "let x = \"hello\".to_string();";
    let pattern_str = "$LITERAL.to_string()";
    let replacement_str = "$LITERAL.into()";
    let lang = Language::from_str("rust").unwrap();
    println!("Testing ast-grep-core directly:");
    println!("  Code: {code}");
    println!("  Pattern: {pattern_str}");
    println!("  Replacement: {replacement_str}");
    let mut ast = AstGrep::new(code, lang);
    let pattern = Pattern::new(pattern_str, lang);
    let matches: Vec<_> = ast.root().find_all(pattern.clone()).collect();
    println!("  Matches found: {}", matches.len());
    for m in matches {
        println!("    Match text: {}", m.text());
    }
    let replaced = ast.replace(pattern, replacement_str);
    match replaced {
        Ok(_) => {
            let rewritten_code = ast.root().text().to_string();
            println!("  Replacement successful. Rewritten code: {rewritten_code}");
            assert_eq!(rewritten_code, "let x = \"hello\".into();");
        }
        Err(e) => {
            eprintln!("  Replacement failed: {e:?}");
            panic!("Replacement failed");
        }
    }
    Ok(())
}
