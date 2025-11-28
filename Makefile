.PHONY: fmt clippy test pre-commit clean release docs help test-middleware test-lib pr

# Format code
fmt:
	cargo fmt --all

# Run clippy
clippy:
	cargo clippy --all-targets --all-features -- -D warnings

# Run all tests
test: test-middleware test-lib

test-middleware:
	cargo test -p trace2e_core --all-features
test-lib:
	./test-lib.sh

# Build release binary for trace2e middleware
release:
	cargo build -r -p trace2e_middleware

# Generate documentation for trace2e_core
docs:
	cargo doc -p trace2e_core --no-deps --open

# Run all pre-commit checks
pr: fmt clippy test
	@echo "All pre-commit checks passed!"

# Clean build artifacts
clean:
	cargo clean

# Help target
help:
	@echo "Available targets:"
	@echo "  fmt             - Format code (workspace)"
	@echo "  clippy          - Run clippy for all targets/features (deny warnings)"
	@echo "  test            - Run all tests (middleware + custom lib)"
	@echo "  test-middleware - Run middleware (trace2e_core) tests"
	@echo "  test-lib        - Run custom lib tests (./test-lib.sh)"
	@echo "  release         - Build trace2e_middleware in release mode"
	@echo "  docs            - Generate and open documentation for trace2e_core"
	@echo "  pr              - Run all pre-commit checks (fmt + clippy + test)"
	@echo "  clean           - Clean build artifacts"
	@echo "  help            - Show this help message"