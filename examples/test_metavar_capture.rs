use ast_grep_language::SupportLang as Language;
use ast_grep_mcp::rules::ast::{PatternRule, Rule};
use ast_grep_mcp::rules::evaluation::RuleEvaluator;

fn main() {
    let evaluator = RuleEvaluator::new();

    let code = r#"class MyClass {
    debug() {
        console.log("debugging");
    }
    process() {
        this.data = "processed";
    }
}
"#;

    // Test metavariable capture patterns
    let patterns = vec![
        ("class $CLASS { $METHODS }", "Standard metavar for body"),
        ("class $CLASS { $BODY }", "Different metavar name"),
        ("class $CLASS { $$$ }", "Multi-match wildcard"),
        ("function $NAME() { $BODY }", "Function metavar test"),
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
                    println!("    Captured variables:");
                    for (var_name, var_value) in &m.vars {
                        println!(
                            "      ${}: '{}'",
                            var_name,
                            var_value.lines().next().unwrap_or("")
                        );
                    }
                }
            }
            Err(e) => {
                println!("  Error: {e}");
            }
        }
        println!();
    }

    // Test with simpler code
    let simple_code = "function test() { return 42; }";
    println!("\n--- Testing function pattern ---");
    println!("Code: {simple_code}");

    let func_rule = Rule::Pattern(PatternRule::Simple {
        pattern: "function $NAME() { $BODY }".to_string(),
    });

    match evaluator.evaluate_rule(&func_rule, simple_code, Language::JavaScript) {
        Ok(matches) => {
            println!("Found {} matches", matches.len());
            for m in &matches {
                println!("Variables:");
                for (var_name, var_value) in &m.vars {
                    println!("  ${var_name}: '{var_value}'");
                }
            }
        }
        Err(e) => {
            println!("Error: {e}");
        }
    }
}
