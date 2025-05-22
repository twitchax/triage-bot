## TODO

- Consider adding an "agent per task".  One agent searches web.  One agent returns a message search query.  One agent goes to glean, etc.?  Maybe a good argument for making an "agent abstraction" that wraps the llm client.  That way tools, setup, etc., are all abstracted away.
- Add context to search agent.
- Replace unit tests by factoring out non-client code, and unit testing that; then, tear out `mockall`.
- Add more integration tests.
- Improve documentation.

## Cool Ideas

## Fly Deploy Notes

```bash
fly deploy -c .hidden/fly.toml --local-only
```