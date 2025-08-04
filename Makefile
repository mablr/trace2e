.PHONY: fmt clippy test pre-commit clean

# Format code
fmt:
	cargo fmt

# Run clippy on the middleware package
clippy:
	cargo clippy -p trace2e_middleware

# Run all tests
test:
	cargo test

# Run all pre-commit checks
pr: fmt clippy test
	@echo "All pre-commit checks passed!"

# Clean build artifacts
clean:
	cargo clean

# Help target
help:
	@echo "Available targets:"
	@echo "  fmt        - Format code"
	@echo "  clippy     - Run clippy on trace2e_middleware package"
	@echo "  test       - Run all tests"
	@echo "  pr         - Run all pre-commit checks (fmt + clippy + test)"
	@echo "  clean      - Clean build artifacts"
	@echo "  help       - Show this help message"