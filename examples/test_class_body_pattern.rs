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

    // Test different approaches to matching class structure
    let patterns = [
        ("class $CLASS { $METHODS }", "Original pattern"),
        ("class $CLASS $BODY", "Without braces"),
        ("class_declaration", "Just the node kind"),
    ];

    println!("Code to match:\n{code}");
    println!("\n---\n");

    // Test pattern matches
    for (pattern, desc) in &patterns[..2] {
        println!("Testing pattern '{pattern}' - {desc}");
        let rule = Rule::Pattern(PatternRule::Simple {
            pattern: pattern.to_string(),
        });

        match evaluator.evaluate_rule(&rule, code, Language::JavaScript) {
            Ok(matches) => {
                println!("  Found {} matches", matches.len());
                for (i, m) in matches.iter().enumerate() {
                    println!(
                        "    Match {}: '{}'\n",
                        i + 1,
                        m.text.lines().next().unwrap_or("")
                    );
                    println!("    Variables:");
                    for (var_name, var_value) in &m.vars {
                        println!("      ${}: {} chars", var_name, var_value.len());
                    }
                }
            }
            Err(e) => {
                println!("  Error: {e}");
            }
        }
        println!();
    }

    // Test kind match
    println!("Testing kind 'class_declaration'");
    let kind_rule = Rule::Kind("class_declaration".to_string());
    match evaluator.evaluate_rule(&kind_rule, code, Language::JavaScript) {
        Ok(matches) => {
            println!("  Found {} matches", matches.len());
        }
        Err(e) => {
            println!("  Error: {e}");
        }
    }
}
