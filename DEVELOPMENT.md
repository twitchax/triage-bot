## TODO

- Threads in slack could be used to keep the conversation going.  So, we could correlate the `thread_ts` to the _first_ LLM request id, and then use that to make subsequent requests.
  Would likely save money on the OpenAI API, and also make it easier to follow conversations.

## Cool Ideas

## Fly Deploy Notes

```bash
fly deploy -c .hidden/fly.toml --local-only
```