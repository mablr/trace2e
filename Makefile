.PHONY: fmt clippy test pre-commit clean

# Format code
fmt:
	cargo +nightly fmt

# Run clippy
clippy:
	cargo clippy --all-targets --all-features -- -D warnings

# Run all tests
test: test-middleware test-lib

test-middleware:
	cargo test -p trace2e_core
test-lib:
	./test-lib.sh

# Run all pre-commit checks
pr: fmt clippy test
	@echo "All pre-commit checks passed!"

# Clean build artifacts
clean:
	cargo clean

# Help target
help:
	@echo "Available targets:"
	@echo "  fmt             - Format code"
	@echo "  clippy          - Run clippy on trace2e_middleware package"
	@echo "  test            - Run all tests (middleware + custom lib)"
	@echo "  test-middleware - Run middleware tests"
	@echo "  test-lib        - Run custom lib tests"
	@echo "  pr              - Run all pre-commit checks (fmt + clippy + test)"
	@echo "  clean           - Clean build artifacts"
	@echo "  help       - Show this help message"