use ast_grep_language::SupportLang as Language;
use ast_grep_mcp::rules::ast::{PatternRule, Rule};
use ast_grep_mcp::rules::evaluation::RuleEvaluator;

fn main() {
    let evaluator = RuleEvaluator::new();

    // This is the exact code from the failing test
    let code = r#"class MyClass {
    // This should match: method_definition inside class with console.log but no return
    debug() {
        console.log("debugging");
        this.process();
    }

    // This should NOT match: has return statement
    calculate() {
        console.log("calculating");
        return 42;
    }

    // This should NOT match: no console.log
    process() {
        this.data = "processed";
    }
}

// This should NOT match: not inside a class
function standalone() {
    console.log("standalone");
}
"#;

    // Find the class - try different patterns
    let patterns = vec![
        "class $CLASS { $METHODS }",
        "class $CLASS { $$$ }",
        "class MyClass { $$$ }",
    ];

    for pattern in &patterns {
        println!("\nTrying pattern: '{pattern}'");
        let class_rule = Rule::Pattern(PatternRule::Simple {
            pattern: pattern.to_string(),
        });

        let matches = evaluator
            .evaluate_rule(&class_rule, code, Language::JavaScript)
            .unwrap();
        println!("  Found {} matches", matches.len());
    }

    // Use the working pattern
    let class_rule = Rule::Pattern(PatternRule::Simple {
        pattern: "class $CLASS { $$$ }".to_string(),
    });

    let class_matches = evaluator
        .evaluate_rule(&class_rule, code, Language::JavaScript)
        .unwrap();
    println!("Class matches: {}", class_matches.len());
    for m in &class_matches {
        println!(
            "  Class at lines {}-{}, cols {}-{}",
            m.start_line, m.end_line, m.start_col, m.end_col
        );
    }

    // Find method_definitions
    let method_rule = Rule::Kind("method_definition".to_string());
    let method_matches = evaluator
        .evaluate_rule(&method_rule, code, Language::JavaScript)
        .unwrap();
    println!("\nMethod matches: {}", method_matches.len());
    for m in &method_matches {
        println!(
            "  Method '{}' at lines {}-{}, cols {}-{}",
            m.text.lines().next().unwrap_or(""),
            m.start_line,
            m.end_line,
            m.start_col,
            m.end_col
        );
    }

    // Check containment manually
    println!("\nContainment check:");
    if !class_matches.is_empty() && !method_matches.is_empty() {
        let container = &class_matches[0];
        for candidate in &method_matches {
            // Reproduce the is_match_contained_in logic
            let contained = if container.start_line < candidate.start_line
                && candidate.end_line <= container.end_line
            {
                true
            } else if container.start_line == candidate.start_line
                && candidate.end_line <= container.end_line
            {
                container.start_col <= candidate.start_col
            } else if container.start_line < candidate.start_line
                && candidate.end_line == container.end_line
            {
                candidate.end_col <= container.end_col
            } else if container.start_line == candidate.start_line
                && candidate.end_line == container.end_line
            {
                container.start_col <= candidate.start_col && candidate.end_col <= container.end_col
            } else {
                false
            };

            println!(
                "  Method '{}' contained in class: {}",
                candidate.text.lines().next().unwrap_or(""),
                contained
            );

            // Debug the comparison
            println!(
                "    Container: lines {}-{}, cols {}-{}",
                container.start_line, container.end_line, container.start_col, container.end_col
            );
            println!(
                "    Candidate: lines {}-{}, cols {}-{}",
                candidate.start_line, candidate.end_line, candidate.start_col, candidate.end_col
            );
        }
    }
}
