#!/bin/bash
# Script to verify how ast-grep handles duplicate rule IDs

set -e

echo "=== Testing ast-grep duplicate rule ID behavior ==="

# Create a temporary test directory
TEST_DIR=$(mktemp -d)
cd "$TEST_DIR"

echo "Test directory: $TEST_DIR"

# Create sgconfig.yml
cat > sgconfig.yml << 'EOF'
ruleDirs:
  - ./rules1
  - ./rules2
  - ./rules3
EOF

# Create directories
mkdir -p rules1 rules2 rules3 src

# Create the same rule with different content in each directory
# Rules1 - error severity
cat > rules1/test-rule.yml << 'EOF'
id: test-duplicate
language: javascript
message: Rule from rules1 directory
severity: error
rule:
  pattern: console.log("from rules1")
EOF

# Rules2 - warning severity
cat > rules2/test-rule.yml << 'EOF'
id: test-duplicate
language: javascript
message: Rule from rules2 directory
severity: warning
rule:
  pattern: console.log("from rules2")
EOF

# Rules3 - info severity
cat > rules3/test-rule.yml << 'EOF'
id: test-duplicate
language: javascript
message: Rule from rules3 directory
severity: info
rule:
  pattern: console.log("from rules3")
EOF

# Create unique rules in each directory
cat > rules1/unique1.yml << 'EOF'
id: unique-rule-1
language: javascript
message: Unique rule from rules1
rule:
  pattern: unique1()
EOF

cat > rules2/unique2.yml << 'EOF'
id: unique-rule-2
language: javascript
message: Unique rule from rules2
rule:
  pattern: unique2()
EOF

# Create test JavaScript files
cat > src/test1.js << 'EOF'
console.log("from rules1");
console.log("from rules2");
console.log("from rules3");
unique1();
unique2();
EOF

cat > src/test2.js << 'EOF'
function test() {
    console.log("from rules1");
    console.log("from rules2");
}
EOF

echo -e "\n=== Testing with ast-grep scan ==="

# Check if ast-grep is installed
if ! command -v ast-grep &> /dev/null; then
    echo "ast-grep is not installed. Please install it to run this test."
    echo "Visit: https://ast-grep.github.io/guide/quick-start.html"
    exit 1
fi

echo -e "\n1. Running ast-grep scan to see which rules are loaded:"
ast-grep scan --json 2>/dev/null | jq -r '.[] | "[\(.severity)] \(.rule_id): \(.message)"' | sort -u || true

echo -e "\n2. Checking what ast-grep scan reports for duplicate rule:"
# Run scan and filter for our duplicate rule
ast-grep scan --json 2>/dev/null | jq -r '.[] | select(.rule_id == "test-duplicate") | "[\(.severity)] \(.rule_id): \(.message) (matched: \(.text))"' || true

echo -e "\n3. Testing with specific rule ID:"
# Try to run specific rule by ID
echo "Running: ast-grep scan --rule test-duplicate"
ast-grep scan --rule test-duplicate 2>&1 || true

echo -e "\n4. Listing all available rules:"
# Some versions of ast-grep might have a command to list rules
ast-grep scan --list-rules 2>&1 || echo "(list-rules command not available)"

echo -e "\n5. Checking rule loading with verbose/debug output:"
# Try with debug output to see rule loading behavior
AST_GREP_LOG=debug ast-grep scan 2>&1 | grep -i "rule\|load\|duplicate" | head -20 || true

echo -e "\n=== Summary ==="
echo "Based on the test results above, we can determine:"
echo "1. How ast-grep handles duplicate rule IDs"
echo "2. Which rule takes precedence when duplicates exist"
echo "3. Whether all rules are loaded or if duplicates are skipped"

# Cleanup
cd ..
rm -rf "$TEST_DIR"
