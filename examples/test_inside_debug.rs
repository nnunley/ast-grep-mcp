use ast_grep_language::SupportLang as Language;
use ast_grep_mcp::rules::ast::{PatternRule, Rule};
use ast_grep_mcp::rules::evaluation::RuleEvaluator;
use ast_grep_mcp::rules::types::RuleObject;

#[tokio::main]
async fn main() {
    let evaluator = RuleEvaluator::new();

    let code = r#"
class MyClass {
    debug() {
        console.log("debugging");
    }

    process() {
        this.data = "processed";
    }
}

function standalone() {
    console.log("standalone");
}
"#;

    // Test 1: Find class pattern
    println!("\n=== Test 1: Find class pattern ===");
    let class_rule = Rule::Pattern(PatternRule::Simple {
        pattern: "class $CLASS { $METHODS }".to_string(),
    });

    let class_matches = evaluator
        .evaluate_rule(&class_rule, code, Language::JavaScript)
        .unwrap();
    println!("Found {} class matches:", class_matches.len());
    for m in &class_matches {
        println!(
            "  Class at lines {}-{}, cols {}-{}",
            m.start_line, m.end_line, m.start_col, m.end_col
        );
        println!("  First line: {}", m.text.lines().next().unwrap_or(""));
    }

    // Test 2: Find method_definitions
    println!("\n=== Test 2: Find method_definitions ===");
    let method_rule = Rule::Kind("method_definition".to_string());

    let method_matches = evaluator
        .evaluate_rule(&method_rule, code, Language::JavaScript)
        .unwrap();
    println!("Found {} method_definitions:", method_matches.len());
    for m in &method_matches {
        println!(
            "  Method at lines {}-{}, cols {}-{}",
            m.start_line, m.end_line, m.start_col, m.end_col
        );
        println!("  First line: {}", m.text.lines().next().unwrap_or(""));
    }

    // Test 3: Check containment manually
    println!("\n=== Test 3: Check containment ===");
    if !class_matches.is_empty() && !method_matches.is_empty() {
        let class_match = &class_matches[0];
        for method in &method_matches {
            let contained = class_match.start_line <= method.start_line
                && method.end_line <= class_match.end_line;
            println!(
                "  Method '{}' contained in class: {}",
                method.text.lines().next().unwrap_or(""),
                contained
            );
        }
    }

    // Test 4: Use the all rule
    println!("\n=== Test 4: Using all rule ===");
    let yaml = r#"
all:
  - kind: method_definition
  - inside:
      pattern: class $CLASS { $METHODS }
"#;

    let rule_obj: RuleObject = serde_yaml::from_str(yaml).unwrap();
    let all_rule = Rule::from(rule_obj);

    println!("Rule structure: {all_rule:#?}");

    let all_matches = evaluator
        .evaluate_rule(&all_rule, code, Language::JavaScript)
        .unwrap();
    println!("\nFound {} matches with all rule:", all_matches.len());
    for m in &all_matches {
        println!("  Match: {}", m.text.lines().next().unwrap_or(""));
    }
}
