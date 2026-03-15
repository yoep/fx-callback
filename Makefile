.PHONY: cargo

prerequisites: ## Install the Cargo requirements for testing
	@command -v cargo-nextest >/dev/null 2>&1 || cargo install cargo-nextest --locked
	@command -v cargo-llvm-cov >/dev/null 2>&1 || cargo install cargo-llvm-cov

test: prerequisites ## Test the cargo project
	@cargo llvm-cov --lcov --output-path target/lcov.info nextest --all-features

cov: ## Test the cargo project with coverage reporting to stdout
	@cargo llvm-cov nextest --all-features

build: ## Build the cargo project
	@cargo build

build-release: ## Build the release version of the cargo project
	@cargo build --release

bump-%: ## Bump the (major, minor, patch) version of the application
	@bump-my-version bump $*
