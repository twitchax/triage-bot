## TODO

- Change "update channel directive" and "update context" to MCP tools.
- Add unit tests (likely use `mockall` for clients).
- Add integration tests.
- Add CI build of binary.
- Add code coverage in CI.
- Cleanup big methods (LLM completions should split out tool creation, etc.).
- Improve documentation.

## Fly Deploy Notes

```bash
fly deploy -c .hidden/fly.toml --local-only
```