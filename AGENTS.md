# Agents Manifest

> Purpose: Provide a single, opinionated prompt for any LLM‑powered agent (GitHub Copilot Agents, Azure AI Foundry Service, Anthropic Claude tools, etc.) that will interact with Aaron's codebases.
> Scope: All current and future projects that follow Aaron's standard Rust‑centric engineering conventions—regardless of domain or repository.

## Global Guidance

- Read the code first.  src/ and the test suites are the ground truth—comments and READMEs can lag behind reality.
- Always consult the public internet before answering questions or taking actions:
- Pull the latest release notes from GitHub.
- Skim authoritative docs (e.g., docs.rs, official RFCs, language release notes) for breaking changes.

- Search blogs and issue trackers for fresh work‑arounds or emerging best practices.

- Cite sources inline using Markdown links so humans can audit your chain of thought.  Prefer primary sources over blogs.

- Surface trade‑offs explicitly—performance, safety, and security matter.  Highlight any unsafe blocks and cryptographic assumptions.

- Default build profile is release + LTO + mold linker + cargo build -j 64.  Deviate only with a comment explaining why.

- Adopt Aaron’s voice: concise, direct, technically precise, with a sprinkle of dry humor when appropriate.

## Repository Style & Layout

- `src/`: Library & binary code—modules clearly named

- `benches/`: Criterion benchmarks for perf regressions

- `tests/`: Integration tests using cargo nextest

- `ci/` or `.github/`: Build, lint, and release workflows

- `examples/`: Minimal, runnable demos

_Crates should be feature‑gated, follow semantic versioning, and compile cleanly on the latest stable and nightly._

## Build & CI Conventions

```
# Fast, deterministic release build
CARGO_PROFILE_RELEASE_LTO=true \
RUSTFLAGS="-C linker=clang -C link-arg=-fuse-ld=mold" \
cargo build --release -j 64

- CI runs on GitHub Actions.  Keep the badge green.

- Prefer `cargo nextest run` for faster, flaky‑test‑resilient suites.

- Use a local `sccache` with for repeat builds.

## Agent Best Practices

- Generating new code: Respect existing module hierarchy; add tests + benches.

- Answering technical questions: Quote authoritative docs; link inline; highlight gotchas.

- Refactoring suggestions: Include risk assessment, migration steps, and perf impact.

- Performance investigations: Profile first (cargo flamegraph), then propose fixes.

When uncertain about *anything* from a CLI flag to a trait bound—conduct a fresh web search before answering.  Aaron expects cutting‑edge, factual responses every time.

## License

Unless otherwise noted, projects use the MIT License.  Include headers or LICENSE files as required.