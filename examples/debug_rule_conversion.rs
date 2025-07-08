use ast_grep_mcp::rules::ast::Rule;
use ast_grep_mcp::rules::types::RuleObject;

fn main() {
    // Test how the complex rule is being parsed
    let yaml = r#"
all:
  - kind: method_definition
  - has:
      pattern: console.log($MSG)
  - inside:
      pattern: class $CLASS { $METHODS }
  - not:
      has:
        pattern: return $VALUE
"#;

    let rule_obj: RuleObject = serde_yaml::from_str(yaml).unwrap();
    let rule = Rule::from(rule_obj);

    println!("Converted rule: {rule:#?}");

    // Now test just the inside rule part
    let inside_yaml = r#"
inside:
  pattern: class $CLASS { $METHODS }
"#;

    let inside_obj: RuleObject = serde_yaml::from_str(inside_yaml).unwrap();
    let inside_rule = Rule::from(inside_obj);

    println!("\nInside rule alone: {inside_rule:#?}");
}
