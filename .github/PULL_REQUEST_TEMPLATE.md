## Summary

<!-- What changed and why? Keep it user-visible. -->

## Related issue

<!-- Fixes #123, refs #456, or "none". -->

## Verification

- [ ] `npm run format:check`
- [ ] `npm run lint`
- [ ] `npm run typecheck`
- [ ] `npm test`
- [ ] `npm run build`
- [ ] `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- [ ] `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings`
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --locked`

## UI changes

<!-- Screenshots for light and dark themes, plus the minimum window size when layout changed. -->

## Checklist

- [ ] The change is focused and does not include unrelated formatting churn.
- [ ] No LAPS password, sign-in password, token or real domain name appears in
      code, logs, fixtures, screenshots or this description.
- [ ] New behavior has a regression test where practical.
- [ ] New or changed strings exist in both `src/i18n/ru.ts` and `src/i18n/en.ts`.
- [ ] IPC contract changes update `src/types.ts`, `src/lib/mockIpc.ts` and the
      Rust serde model in the same change.
- [ ] No generated files (`dist`, `node_modules`, `src-tauri/target`,
      `src-tauri/gen`) are committed.
