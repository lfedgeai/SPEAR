# Frontend UI Tests (Playwright)

## Location & Run

- Directory: `spear/ui-tests`
- Run: `npm test`
- Tests compile and run the embedded SMS with WebAdmin at `127.0.0.1:8081`

## Global Setup

- `global-setup.ts` cleans `data/files` before each run to ensure deterministic file list, upload, and delete tests

## Key Specs

- `task_modal.spec.ts`: select `sms+file`, click `Use` in picker to fill URI
- `task_modal_scheme_prefill.spec.ts`: scheme selection pre-fills `Executable URI`
- `executable_select_unit.spec.ts`: uses hidden native `select` for stable type selection
- `files.spec.ts`: upload a small file and copy URI
- `files_delete.spec.ts`: delete a file and verify the list updates
- `files_modified_tz.spec.ts`: human-readable timestamps in selected timezone

## Environment

- Playwright config: `playwright.config.ts`
- Browser: Chromium by default (configurable)

## Notes

- Dropdown stability: tests use a native `select` mirror to avoid portal positioning flakiness
- Executable Type provides a native `select` mirror for accessibility and reliable automation
- Delete verification: prefer row-matching and message assertion over global row counts

## CI

- Can run in CI; refer to `ui-tests/package.json` scripts
- Logs and artifacts: failed runs produce context files under `test-results`
