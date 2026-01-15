# Documentation guidelines

## Goals

- Keep docs easy to navigate and consistent across the repo.
- Prefer bilingual docs (English + Chinese) for shared concepts.
- Make docs durable: link to code by relative paths and avoid stale “line-number-only” references.

## Language and naming

- English: `{topic}-en.md`
- Chinese: `{topic}-zh.md`
- If a doc is intentionally bilingual in one file, name it without a suffix (rare; prefer split files).

## Structure

- Start with a short “What is this doc for” section.
- Include code references using relative links (e.g. `../src/...`).
- For operational docs, include a “How to verify” section.

## When to update docs

- Any API/ABI change that affects:
  - hostcalls
  - CLI flags / configuration
  - HTTP/gRPC endpoints
- Any semantic change where earlier docs would mislead a reader.

## Style

- Avoid embedding secrets or tokens.
- Prefer concrete examples.
- Keep the English and Chinese versions aligned in section numbering.

