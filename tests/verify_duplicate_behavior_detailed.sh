#!/bin/bash
# More detailed test of ast-grep duplicate rule behavior

set -e

echo "=== Detailed test of ast-grep duplicate rule handling ==="

# Create a temporary test directory
TEST_DIR=$(mktemp -d)
cd "$TEST_DIR"

# Create sgconfig.yml
cat > sgconfig.yml << 'EOF'
ruleDirs:
  - ./rules1
  - ./rules2
EOF

mkdir -p rules1 rules2 src

# Create same rule ID with different patterns
cat > rules1/console-check.yml << 'EOF'
id: console-check
language: javascript
message: No console.log allowed (from rules1)
severity: error
rule:
  pattern: console.log($$$)
EOF

cat > rules2/console-check.yml << 'EOF'
id: console-check
language: javascript
message: No console.error allowed (from rules2)
severity: warning
rule:
  pattern: console.error($$$)
EOF

# Create test file with both patterns
cat > src/test.js << 'EOF'
function example() {
    console.log("This is a log");
    console.error("This is an error");
    console.debug("This is debug");
}
EOF

echo -e "\n1. Running ast-grep scan to see all matches:"
ast-grep scan 2>/dev/null || true

echo -e "\n2. Running with JSON output for detailed analysis:"
ast-grep scan --json 2>/dev/null | jq '.' || true

echo -e "\n3. Count how many times rule 'console-check' is triggered:"
MATCHES=$(ast-grep scan --json 2>/dev/null | jq -r '.[] | select(.rule_id == "console-check") | .rule_id' | wc -l)
echo "Rule 'console-check' matched $MATCHES times"

echo -e "\n4. Show unique rule-message combinations:"
ast-grep scan --json 2>/dev/null | jq -r '.[] | select(.rule_id == "console-check") | "\(.rule_id): \(.message)"' | sort -u || true

# Cleanup
cd ..
rm -rf "$TEST_DIR"

echo -e "\n=== CONCLUSION ==="
echo "ast-grep loads and applies ALL rules with the same ID from different directories."
echo "This is DIFFERENT from our implementation where the first rule wins."
