use ast_grep_mcp::rules::ast::Rule;
use ast_grep_mcp::rules::types::RuleObject;

fn main() {
    // Test how the inside rule with pattern is parsed from YAML
    let yaml = r#"
inside:
  pattern: class $CLASS { $METHODS }
"#;

    println!("Testing YAML parsing for inside rule");
    println!("YAML:\n{yaml}");

    match serde_yaml::from_str::<RuleObject>(yaml) {
        Ok(rule_obj) => {
            println!("\nParsed RuleObject: {rule_obj:#?}");

            // Convert to Rule enum
            let rule = Rule::from(rule_obj);
            println!("\nConverted to Rule enum: {rule:#?}");
        }
        Err(e) => {
            println!("Failed to parse: {e}");
        }
    }

    // Test the all rule structure
    let all_yaml = r#"
all:
  - kind: method_definition
  - inside:
      pattern: class $CLASS { $METHODS }
"#;

    println!("\n\nTesting YAML parsing for all rule with inside");
    println!("YAML:\n{all_yaml}");

    match serde_yaml::from_str::<RuleObject>(all_yaml) {
        Ok(rule_obj) => {
            println!("\nParsed RuleObject: {rule_obj:#?}");

            // Convert to Rule enum
            let rule = Rule::from(rule_obj);
            println!("\nConverted to Rule enum: {rule:#?}");
        }
        Err(e) => {
            println!("Failed to parse: {e}");
        }
    }
}
