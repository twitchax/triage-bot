## TODO

- Fix the search agent to properly handle the database lookup.
- Replace unit tests by factoring out non-client code, and unit testing that; then, tear out `mockall`.
- Add more integration tests.
- Improve documentation.
- The surreal `Any` type + `Mem` bloats the binary by 30 MB-ish.  Consider splitting it into `cfg(test)` only. 

## Cool Ideas

## Fly Deploy Notes

```bash
fly deploy -c .hidden/fly.toml --local-only
```