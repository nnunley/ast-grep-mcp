use ast_grep_language::SupportLang as Language;
use ast_grep_mcp::rules::ast::{PatternRule, Rule};
use ast_grep_mcp::rules::evaluation::RuleEvaluator;

fn main() {
    let evaluator = RuleEvaluator::new();

    let code = r#"class MyClass {
    debug() {
        console.log("debugging");
    }
}
"#;

    // Test different patterns
    let patterns = vec![
        ("class $CLASS { $$$}", "Multi-line with spaces"),
        ("class $CLASS {$$$}", "No space before brace"),
        ("class $_ { $$$ }", "Wildcard metavar"),
        ("class MyClass { $$$ }", "Exact class name"),
    ];

    println!("Code to match:\n{code}");
    println!("\n---\n");

    for (pattern, desc) in patterns {
        println!("Testing pattern '{pattern}' - {desc}");
        let rule = Rule::Pattern(PatternRule::Simple {
            pattern: pattern.to_string(),
        });

        match evaluator.evaluate_rule(&rule, code, Language::JavaScript) {
            Ok(matches) => {
                println!("  Found {} matches", matches.len());
                for (i, m) in matches.iter().enumerate() {
                    println!("    Match {}: lines {}-{}", i + 1, m.start_line, m.end_line);
                    println!("    Text: '{}'\n", m.text.trim());
                }
            }
            Err(e) => {
                println!("  Error: {e}");
            }
        }
        println!();
    }
}
