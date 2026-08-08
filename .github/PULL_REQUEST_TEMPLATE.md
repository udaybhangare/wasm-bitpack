## What does this change and why

<!-- Describe the change and the motivation behind it. -->

## Checklist

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo test --workspace` passes (`cargo xtask ci` runs all three of the above)
- [ ] Every new `pub` item has a `///` doc comment
- [ ] Every new `unsafe` block has a `// SAFETY:` comment justifying it at that call site
      (if this PR touches `wasm128/`, called out explicitly for reviewer focus)
- [ ] `CHANGELOG.md`'s `[Unreleased]` section has an entry, if this change is user-visible
