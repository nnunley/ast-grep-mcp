use ast_grep_language::SupportLang as Language;
use ast_grep_mcp::rules::ast::{PatternRule, Rule};
use ast_grep_mcp::rules::evaluation::RuleEvaluator;

fn main() {
    let evaluator = RuleEvaluator::new();

    let code = r#"
class MyClass {
    debug() {
        console.log("debugging");
    }
}
"#;

    // Test different class patterns
    let patterns = vec![
        "class $CLASS { $METHODS }",
        "class $CLASS { $$$ }",
        "class MyClass { $$$ }",
        "class $_ { $$$ }",
        "class $NAME { $$$ }",
    ];

    for pattern in patterns {
        println!("\nTesting pattern: '{pattern}'");
        let rule = Rule::Pattern(PatternRule::Simple {
            pattern: pattern.to_string(),
        });

        match evaluator.evaluate_rule(&rule, code, Language::JavaScript) {
            Ok(matches) => {
                println!("  Found {} matches", matches.len());
                for m in &matches {
                    println!(
                        "    Lines {}-{}: {}",
                        m.start_line,
                        m.end_line,
                        m.text.lines().next().unwrap_or("")
                    );
                }
            }
            Err(e) => {
                println!("  Error: {e}");
            }
        }
    }
}
