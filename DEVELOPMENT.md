## TODO

- Change "update channel directive" and "update context" to MCP tools.
- Add unit tests (likely use `mockall` for clients).
- Add CI build of binary.
- Add code coverage in CI.
- Cleanup big methods (LLM completions should split out tool creation, etc.).
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

## Fly Deploy Notes

```bash
fly deploy -c .hidden/fly.toml --local-only
```