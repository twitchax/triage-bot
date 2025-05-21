## TODO

- Add better unit tests (likely use `mockall` for clients).
- Improve documentation.

## Integration Testing

The project includes integration tests in the `tests/integration.rs` file. These tests demonstrate:

1. Database operations with the in-memory SurrealDB
2. Chat event handling with a test LLM client
3. OpenAI integration (only runs if OPENAI_API_KEY is set)

To run the integration tests locally:
```bash
cargo test --test integration
```

To run a specific integration test:
```bash
cargo test --test integration test_db_integration
```

### CI Integration Testing

In CI, integration tests are run using [cargo-nextest](https://nexte.st/):

```bash
cargo nextest run
```

### Required Environment Variables for CI

For the tests to run successfully in CI, the following environment variables should be set:

- `OPENAI_API_KEY`: Required for the OpenAI integration test. If not provided, the test will be skipped.
- `CODECOV_TOKEN`: Required for uploading code coverage results to Codecov.

Set these as GitHub Actions secrets in the repository settings.

## Cool Ideas

- Have multiple agents: essentially, have a `gpt-4.1` agent (with its own prompt) that calls tools for context, and prepares a "report" for a reasoning model like `o3` to consume (with its own prompt).

## Fly Deploy Notes

```bash
fly deploy -c .hidden/fly.toml --local-only
```