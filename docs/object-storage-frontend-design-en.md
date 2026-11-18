# Object Storage Frontend Redesign

This document describes how `web-admin` should be redesigned around the new object storage architecture when breaking changes are acceptable.

The target backend architecture is now:

- `ObjectCatalog` for metadata
- `BlobStore` for blob content
- `ObjectService` for object lifecycle orchestration
- `ObjectJanitor` for background cleanup and failure convergence

The frontend should stop evolving around the old `Files + legacy file URI` model. Instead, it should align directly with the new object domain so that UI structure, API shape, and user mental model remain consistent.

Related documents:

- [rustfs-object-storage-design-en.md](./rustfs-object-storage-design-en.md)
- [web-admin-ui-guide-en.md](./web-admin-ui-guide-en.md)
- [web-admin-overview-en.md](./web-admin-overview-en.md)
- [task-api-refactor-en.md](./task-api-refactor-en.md)

## Goals

The goals of this frontend redesign are:

- make objects first-class resources in the frontend instead of hidden backend implementation details
- make the frontend information architecture map directly to the new object system
- remove the old `Files` mental model and legacy file URI dependency
- let task creation, object browsing, and object operations share one consistent object reference model
- avoid leaking RustFS and other backend implementation details into normal product UI
- keep interactions clear, intuitive, and maintainable instead of layering compatibility paths

## Non-Goals

This design does not attempt to:

- expose every low-level RustFS operational parameter in the frontend
- implement a complex inline object preview system in phase one
- keep `Files` and `Objects` as long-term parallel concepts
- preserve old boolean-driven object API semantics in the frontend

## Core Conclusions

### 1. The Frontend Resource Model Should Move From `Files` To `Objects`

Once breaking change is acceptable, the frontend should stop treating files as the primary resource while objects remain hidden implementation details.

The recommended direction is:

- remove `Files` as a primary navigation concept
- add `Objects` as the new primary resource page
- replace the file picker in task creation with an object picker

### 2. The Frontend Should Not Configure RustFS Directly

The frontend should be centered on the object system, not on blob backend internals.

Therefore:

- normal management pages should not edit RustFS bucket, path, or timeout details directly
- the frontend may show read-only object storage health and summary information
- RustFS remains a runtime backend concern handled primarily by backend configuration and deployment

### 3. Task Resources Should No Longer Use Legacy File URIs

Task creation should align directly with the new object system instead of reusing the old file URI model.

Recommended direction:

- deprecate the legacy file URI scheme
- stop exposing handwritten URI input as the main task resource interaction
- introduce a structured object reference model

## Frontend Information Architecture

### New Top-Level Navigation

The `web-admin` main navigation should become:

- Dashboard
- Nodes
- Tasks
- Executions
- Objects
- AI Backends
- MCP
- Settings

Where:

- the `Files` page is removed
- the `Objects` page becomes the main object domain entry

### Responsibilities Of The `Objects` Module

The `Objects` module should cover:

- browse object list
- inspect object summaries
- upload objects
- download objects
- delete objects
- retain / release
- pin / unpin
- open object details

### Object Storage Status In `Settings`

A read-only `Object Storage` section should be added to `Settings` or node detail pages to show:

- current object storage backend type
- health status
- endpoint summary
- bucket and root summary
- recent error state

Low-level backend configuration should not be editable from ordinary management UI.

## Frontend Domain Model

The frontend should define its own object domain types instead of reusing the old file models.

Suggested types:

```ts
export type ObjectState = "ready" | "deleting";

export interface ObjectSummary {
  key: string;
  size: number;
  checksum: string;
  refCount: number;
  pinned: boolean;
  state: ObjectState;
  updatedAt: string;
  metadata: Record<string, string>;
}

export interface ObjectDetail extends ObjectSummary {
  createdAt: string;
}

export interface ListObjectsRequest {
  prefix?: string;
  limit?: number;
  pageToken?: string;
}

export interface ListObjectsResponse {
  items: ObjectSummary[];
  nextPageToken?: string;
}
```

Requirements:

- list and detail models are organized around metadata
- list responses never include object content
- all state fields align directly with the new backend model

## API Design

### Frontend APIs Should Align Directly With The New Backend Protocol

Since breaking change is accepted, the frontend should not build a compatibility wrapper around the old object API.

Add a dedicated module:

- `web-admin/src/api/objects.ts`

Suggested exported methods:

- `listObjects`
- `headObject`
- `getObject`
- `putObject`
- `deleteObject`
- `retainObject`
- `releaseObject`
- `pinObject`
- `unpinObject`
- `getObjectStorageStatus`

### Old Semantics To Reject

The new frontend design should completely remove:

- `include_value`
- `include_values`
- `start_after`
- `add_ref`
- `remove_ref`

These semantics should disappear from the frontend API layer itself, not just be ignored in UI code.

### Query And Cache Strategy

Use React Query as the unified cache model for the object domain.

Suggested query keys:

- `["objects", prefix, pageToken, limit]`
- `["object", key]`
- `["object-storage-status"]`

Suggested invalidation strategy:

- `putObject` invalidates object lists and object detail
- `deleteObject` invalidates object lists and clears detail cache
- `retain/release/pin/unpin` prefer local updates first, then invalidate detail and list

## Page Design

### Objects List Page

Add:

- `web-admin/src/features/objects/ObjectsPage.tsx`

Responsibilities:

- query objects by prefix
- show object summary rows
- support upload and object-level actions

Recommended columns:

- `Key`
- `Size`
- `Checksum`
- `Refs`
- `Pinned`
- `State`
- `Updated At`

Recommended row actions:

- `Details`
- `Download`
- `Pin` / `Unpin`
- `Retain`
- `Release`
- `Delete`

Design principles:

- list shows metadata only
- object content is never rendered directly in the list
- delete should be disabled when clearly forbidden, or fail with an explicit message

### Object Detail Page

Add:

- `web-admin/src/features/objects/ObjectDetailPage.tsx`

Recommended page sections:

1. Summary
2. Metadata
3. Actions

Recommended content:

- key
- size
- checksum
- ref count
- pinned
- state
- created at
- updated at
- metadata key-value list

Recommended actions:

- `Download`
- `Pin` / `Unpin`
- `Retain`
- `Release`
- `Delete`

Requirements:

- default to `headObject`
- call `getObject` only when content is explicitly requested
- never load large object content automatically on page open

### Upload Dialog

Introduce a dedicated object upload dialog instead of reusing the old file upload interaction.

Recommended inputs:

- `Object key`
- `Local file`
- `Custom metadata`
- `Overwrite`

Design principles:

- the user uploads an object, not a file record
- success should return the object key
- the UI may navigate to object detail after upload

## Task Creation Redesign

### Goal

The task creation flow should move from the current `URI string + SMS file chooser` model to a structured object resource model.

### Recommended Direction

Resource input should no longer be modeled primarily as a free-form URI field.

Suggested model:

```ts
type ExecutableSource =
  | { kind: "none" }
  | { kind: "object"; key: string }
  | { kind: "url"; url: string }
  | { kind: "s3"; bucket: string; key: string }
  | { kind: "minio"; bucket: string; key: string };
```

If the backend task protocol also accepts breaking change, it should move to a structured resource model as well instead of continuing to send raw URIs.

### Task Form Changes

The current task form concepts:

- `Executable URI`
- `Choose Local`
- legacy file URI auto-fill

should all be replaced.

Suggested new interaction:

- `Executable Source Type`
  - `None`
  - `Object`
  - `URL`
  - `S3`
  - `MinIO`
- when `Object` is selected:
  - open `ObjectPickerModal`
  - fill back the selected object key
  - show object summary in the form

### `ObjectPickerModal`

Add a reusable component:

- `web-admin/src/features/objects/ObjectPickerModal.tsx`

It should be reusable in:

- task creation
- future forms that depend on object input

Recommended capabilities:

- prefix search
- paginated browsing
- object summary display
- returns `ObjectSummary` on selection

## Routing And Page Structure

Add or replace routes in `AppShell`:

- `/objects`
- `/objects/:key`

And remove:

- `/files`
- `/files/:id`

If a temporary transition is needed, the old routes may redirect on the frontend, but they should not remain as a long-term product concept.

## Suggested Module Organization

To keep object-domain boundaries clean, add:

- `web-admin/src/api/objects.ts`
- `web-admin/src/features/objects/ObjectsPage.tsx`
- `web-admin/src/features/objects/ObjectDetailPage.tsx`
- `web-admin/src/features/objects/ObjectPickerModal.tsx`
- `web-admin/src/features/objects/object-types.ts`
- `web-admin/src/features/objects/object-format.ts`

Task creation should reuse object-domain components instead of duplicating local selection logic.

## Interaction Principles

### 1. Metadata First

The frontend should default to metadata-driven interaction:

- list shows metadata only
- detail defaults to metadata only
- content fetch is always explicit

### 2. Immediate Feedback For Object Actions

The UI should reflect object actions immediately:

- `Pin` / `Unpin` updates the marker
- `Retain` / `Release` updates the ref count
- delete removes the row or marks it as `Deleting`

### 3. Do Not Expose Backend Implementation Details

Normal users do not need to know internal RustFS paths, sharding, or bucket layout.

The frontend should present object semantics, not blob storage internals.

### 4. Semantic Error Messages

The frontend should map common object errors to clear messages such as:

- object not found
- object is being deleted
- object is still referenced and cannot be deleted
- object is pinned and cannot be deleted
- object storage is currently unavailable

Do not fall back to generic `Request failed` messages.

## State And Observability

The frontend should also surface object-domain runtime state:

- object storage health
- backend connectivity errors
- visible `Deleting` state markers

This helps users understand background retries or temporary failures instead of assuming the UI is broken.

## Testing Strategy

Recommended frontend test coverage:

### Unit Tests

- object state formatting
- object action availability rules
- `ObjectPickerModal` selection logic

### Component Tests

- `ObjectsPage`
  - query, pagination, and action state
- `ObjectDetailPage`
  - metadata rendering and action triggers
- task form object selection flow

### End-To-End Tests

- upload object and verify it appears in the list
- open object detail and download content
- retain / release / pin / unpin
- create task using an object as the executable resource

## Migration Plan

### Phase 1: Add The New Object Module

- add `objects.ts`
- add object list and detail pages
- add `Objects` to navigation

### Phase 2: Redesign Tasks

- replace the file picker with an object picker
- remove legacy file URIs from the main interaction path
- move task resource input to structured object selection

### Phase 3: Remove The Files Module

- delete `FilesPage`
- delete `FileDetailPage`
- delete `api/files.ts`
- delete frontend logic related to legacy file URIs

### Phase 4: Converge Docs And Tests

- update the Web Admin guide
- update task creation documentation
- update UI tests

## Risks And Trade-Offs

### Benefits

- frontend and backend object models become aligned
- task creation, object browsing, and object operations share one consistent resource model
- the frontend no longer maintains old file semantics and compatibility layers
- page structure becomes clearer and future extension becomes more natural

### Costs

- navigation, pages, and task creation flows all become breaking changes
- existing user habits around `Files` and legacy file URIs need migration
- frontend APIs, backend protocols, and documentation must all change together

### Judgment

If the backend is already moving to the new object system, these breaking changes are justified. Keeping `Files + legacy file URIs` alive would only preserve old semantics that no longer fit the architecture.

## Conclusion

The recommended frontend direction is:

- replace `Files` with `Objects`
- replace the local file picker with an object picker
- replace legacy file URIs with structured object references
- replace old object API semantics with the new object APIs

This design aligns directly with the new `ObjectCatalog + BlobStore + ObjectService` architecture, keeps boundaries clear, and is easier to maintain over time.
