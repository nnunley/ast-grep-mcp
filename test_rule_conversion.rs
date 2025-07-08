use ast_grep_mcp::rules::ast::Rule;
use ast_grep_mcp::rules::types::RuleObject;
use serde_yaml;

fn main() {
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

    println!("Converted rule: {:#?}", rule);
}
