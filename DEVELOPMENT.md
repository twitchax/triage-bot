## TODO

- Add better unit tests (likely use `mockall` for clients).
- Improve documentation.

## Integration Testing

The project includes integration tests in the `tests/integration.rs` file. These tests demonstrate:

1. Database operations with the in-memory SurrealDB
2. Chat event handling with a test LLM client
3. OpenAI integration (only runs if OPENAI_API_KEY is set)

To run the integration tests:
```bash
cargo test --test integration
```

To run a specific integration test:
```bash
cargo test --test integration test_db_integration
```

## Cool Ideas

- Have multiple agents: essentially, have a `gpt-4.1` agent (with its own prompt) that calls tools for context, and prepares a "report" for a reasoning model like `o3` to consume (with its own prompt).

## Fly Deploy Notes

```bash
fly deploy -c .hidden/fly.toml --local-only
```