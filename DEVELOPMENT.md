## TODO

- Improve documentation.
- The surreal `Any` type + `Mem` bloats the binary by 30 MB-ish.  Consider splitting it into `cfg(test)` only. 

## Cool Ideas

## Fly Deploy Notes

```bash
fly deploy -c .hidden/fly.toml --local-only
```