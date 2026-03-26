#!/bin/bash
set -e

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${GREEN}Starting local verification...${NC}"

# 1. Check formatting
echo -e "\n${GREEN}Checking formatting...${NC}"
cargo fmt --all -- --check

# 2. Run Clippy
echo -e "\n${GREEN}Running Clippy...${NC}"
cargo clippy --workspace --all-targets -- -D warnings

# 3. Build workspace
echo -e "\n${GREEN}Building workspace...${NC}"
cargo build

# 4. Run tests
echo -e "\n${GREEN}Running tests...${NC}"
# We might want to limit parallel jobs for tests too if they are resource intensive
cargo test

# 5. Run examples
echo -e "\n${GREEN}Running examples...${NC}"
cargo run --example simple_example
cargo run --example parallel_query
cargo run --example crud_operations

echo -e "\n${GREEN}Verification successful!${NC}"
