## TODO

- Replace unit tests by factoring out non-client code, and unit testing that; then, tear out `mockall`.
- Add integration tests.
- Improve documentation.

## Cool Ideas

- Have multiple agents: essentially, have a `gpt-4.1` agent (with its own prompt) that calls tools for context, and prepares a "report" for a reasoning model like `o3` to consume (with its own prompt).

## Fly Deploy Notes

```bash
fly deploy -c .hidden/fly.toml --local-only
```