# Frontend UI Tests (Playwright)

## Location & Run

- Directory: `spear/ui-tests`
- Run: `npm test`
- Tests compile and run the embedded SMS with WebAdmin at `127.0.0.1:8081`

## Global Setup

- `global-setup.ts` cleans `data/objects` before each run to ensure deterministic object list, upload, and delete tests

## Key Specs

- `task_modal.spec.ts`: select `object`, click `Use` in picker to fill the object key
- `task_modal_scheme_prefill.spec.ts`: URI source selection pre-fills `Executable URI`
- `executable_select_unit.spec.ts`: uses hidden native `select` for stable type selection
- `objects.spec.ts`: upload a small object and copy the object URI
- `objects_delete.spec.ts`: delete an object and verify the list updates
- `objects_modified_tz.spec.ts`: human-readable timestamps in the selected timezone

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
