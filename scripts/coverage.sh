#!/bin/bash
# Simple script to generate isolated code coverage reports

set -e

echo "Installing cargo-tarpaulin if not present..."
if ! command -v cargo-tarpaulin &> /dev/null; then
    cargo install cargo-tarpaulin
fi

echo "Generating coverage report..."
cargo tarpaulin --verbose

echo "Coverage report generated in coverage/ directory"
echo "Open coverage/tarpaulin-report.html in your browser to view the report"