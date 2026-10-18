# RustFS Object Storage Refactoring Design

## Overview

This document redefines the `spearlet` object storage design under a clear assumption: breaking changes are acceptable if they lead to a cleaner and more elegant architecture.

The core decisions are:

- stop evolving around the current `full object stored in KV` model
- do not disguise RustFS as a `KvStore` backend
- do not add extra layers for hypothetical future use cases
- directly reshape the object subsystem into a simple and maintainable `metadata + blob` architecture

The goal is not to build a system with many abstractions and an unclear spine. The goal is to build a system with a very obvious spine:

- `ObjectCatalog` is the authority for object metadata
- `BlobStore` is the backend for object content
- `ObjectService` orchestrates business lifecycle only
- `ObjectJanitor` handles background cleanup and failure convergence

Related background documents:

- [kv-abstraction-en.md](./kv-abstraction-en.md)
- [kv-factory-pattern-en.md](./kv-factory-pattern-en.md)
- [spearlet-object-service-cleanup-en.md](./spearlet-object-service-cleanup-en.md)
- [spearlet-kv-backend-analysis-en.md](./spearlet-kv-backend-analysis-en.md)

## Design Principles

This design follows these principles:

- when breaking changes are acceptable, boundary correction is more important than preserving old baggage
- the number of abstractions should stay small, keeping only genuinely valuable extension points
- `list`, `head`, and `stats` must operate on metadata only by default
- blob backend differences must stay inside the backend layer and never leak into business logic
- delete, retry, and cleanup must be idempotent
- configuration and assembly should describe the real system structure directly instead of hiding inside a generic `storage` concept

## Problems In The Current Design

The main issue is not a lack of backends. The main issue is that the model itself is not clean enough.

### 1. Object Content And Metadata Are Coupled

The current service stores the following together in one serialized structure:

- `value`
- `metadata`
- `ref_count`
- `pinned`
- timestamps

This causes:

- metadata reads to accidentally pull large payloads
- `list`, stats, and cleanup to slow down with object size
- difficulty moving to a true blob backend

### 2. `KvStore` Semantics Are Not A Good Object Backend Abstraction

`KvStore` is suitable for key-value storage, not for a blob store such as RustFS.

If RustFS is forced into KV semantics, the result is:

- distorted meaning for `scan_prefix`, `range`, and `count`
- unnatural modeling for blob paths, head, and streaming behavior
- increasing backend-specific branching inside the service layer

### 3. API Semantics Are Not Clean Enough

The current object API has several issues:

- `GetObjectRequest.include_value` mixes two behaviors behind one boolean
- `ListObjectsRequest.include_values` turns `list` into a potential bulk-read path
- `start_after` pagination is not stable and behaves more like index-based paging
- `add_ref/remove_ref` are low-level names that do not match the object lifecycle well

### 4. Runtime Assembly Boundaries Are Not Clear

Object storage does not yet have a stable boundary across configuration, services, and backend wiring, which is why the runtime still leans toward directly constructing an in-memory implementation.

## Target Architecture

### Core Conclusion

Keep only 3 core abstractions:

- `ObjectCatalog`
- `BlobStore`
- `ObjectService`

Background cleanup should not become a generic framework. It should stay a concrete component:

- `ObjectJanitor`

Startup assembly should also avoid a separate factory abstraction. A direct builder function is enough:

- `build_object_service(config)`

### Component View

```text
Client
  -> gRPC / HTTP
  -> ObjectService
      -> ObjectCatalog
      -> BlobStore

Background Worker
  -> ObjectJanitor
      -> ObjectCatalog
      -> BlobStore
```

### Responsibilities

- `ObjectCatalog`
  - manages object metadata
  - provides object query, list, and state update capability
  - acts as the authority for object existence and visibility
- `BlobStore`
  - manages content write, read, delete, and head operations
  - hides RustFS and other blob backend differences
- `ObjectService`
  - orchestrates write, read, delete, retain, release, pin, and unpin business rules
  - does not know KV implementation details and does not build RustFS paths directly
- `ObjectJanitor`
  - retries failed deletes
  - cleans orphan blobs
  - converges residual states after temporary failures

## Core Abstractions

### `BlobStore`

`BlobStore` is the only abstraction for object content storage.

Suggested interface:

```rust
#[async_trait]
pub trait BlobStore: Send + Sync + Debug {
    async fn put(&self, req: PutBlobRequest) -> Result<BlobInfo, ObjectError>;
    async fn get(&self, blob_id: &str) -> Result<BlobBytes, ObjectError>;
    async fn head(&self, blob_id: &str) -> Result<BlobInfo, ObjectError>;
    async fn delete(&self, blob_id: &str) -> Result<(), ObjectError>;
}
```

Requirements:

- `put` returns `blob_id`, `size`, and `checksum`
- `head` returns blob metadata only
- `delete` must be idempotent
- in phase one, `get` may return full bytes and evolve to streaming later

### `ObjectCatalog`

`ObjectCatalog` is a better name than `ObjectMetadataStore` because it expresses not only metadata persistence, but also directory-like object semantics.

Suggested interface:

```rust
#[async_trait]
pub trait ObjectCatalog: Send + Sync + Debug {
    async fn get(&self, key: &str) -> Result<Option<ObjectRecord>, ObjectError>;
    async fn put(&self, record: ObjectRecord) -> Result<(), ObjectError>;
    async fn delete(&self, key: &str) -> Result<bool, ObjectError>;
    async fn list(&self, query: ListQuery) -> Result<ListPage<ObjectRecord>, ObjectError>;
    async fn patch(&self, key: &str, patch: ObjectPatch) -> Result<ObjectRecord, ObjectError>;
}
```

Requirements:

- `list` returns metadata only
- `patch` centralizes ref, pin, and state updates
- `ObjectCatalog` is the authority for object visibility
- the implementation may use `KvStore` internally, but that detail must not leak into business logic

### `ObjectService`

`ObjectService` is the orchestration layer, but it should not become a giant all-purpose service object.

Suggested definition:

```rust
pub struct ObjectService {
    catalog: Arc<dyn ObjectCatalog>,
    blobs: Arc<dyn BlobStore>,
    policy: ObjectPolicy,
}
```

Responsibilities:

- validate write requests
- implement overwrite semantics
- query metadata first and load blob content only when needed
- maintain retain/release and pin/unpin lifecycle rules
- drive delete orchestration

### `ObjectJanitor`

There is no need to start with a generic GC framework. A concrete background component is enough.

Suggested definition:

```rust
pub struct ObjectJanitor {
    catalog: Arc<dyn ObjectCatalog>,
    blobs: Arc<dyn BlobStore>,
    policy: ObjectPolicy,
}
```

Its scope should stay narrow:

- retry deletes for `Deleting` objects
- clean orphan blobs

## Data Model

### `ObjectRecord`

`ObjectRecord` should become the single authoritative metadata model in the system.

```rust
pub struct ObjectRecord {
    pub key: String,
    pub blob_id: String,
    pub size: u64,
    pub checksum: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub ref_count: u32,
    pub pinned: bool,
    pub state: ObjectState,
    pub metadata: BTreeMap<String, String>,
}
```

Design notes:

- `ref_count` uses `u32` to avoid negative states
- `metadata` uses `BTreeMap` for stable serialization and tests
- the old mixed `value + metadata` model is removed

### `ObjectState`

The first version should keep only 2 states:

- `Ready`
- `Deleting`

```rust
pub enum ObjectState {
    Ready,
    Deleting,
}
```

`Pending` is intentionally omitted for phase one because:

- there is no clear business need to expose uploading objects
- `Pending` adds visibility and recovery complexity
- temporary write failures can be handled through orphan blob cleanup

### `key` And `blob_id`

These two concepts should stay distinct:

- `key`
  - logical business key
  - used by public APIs
- `blob_id`
  - low-level blob identifier
  - used for RustFS path mapping

Suggested example:

```text
key: workflow/123/output.json
blob_id: objects/ab/cd/550e8400-e29b-41d4-a716-446655440000
```

This makes overwrite, delayed deletion of old versions, and future versioning more natural.

## API Design

Since breaking changes are acceptable, the old ambiguous API behaviors should be removed directly.

### Old Semantics To Remove

- remove `ListObjects.include_values`
- remove `GetObject.include_value`
- remove index-like `start_after`
- remove the low-level `add_ref/remove_ref` naming

### Suggested New API Semantics

- `PutObject`
  - creates or overwrites an object
- `HeadObject`
  - returns object metadata only
- `GetObject`
  - returns object content
- `ListObjects`
  - returns object summaries only
- `DeleteObject`
  - deletes an object
- `RetainObject`
  - increments references
- `ReleaseObject`
  - decrements references
- `PinObject`
  - pins an object
- `UnpinObject`
  - unpins an object

### Why Split `HeadObject` And `GetObject`

Splitting these APIs is cleaner than keeping a boolean such as `include_value`:

- protocol semantics become clearer
- implementation becomes simpler
- HTTP mapping becomes more natural
- one method no longer hides two very different execution paths

### `ListObjects` Returns Summaries Only

`list` should never become a large object read interface.

Therefore:

- `ListObjects` returns `ObjectSummary` only
- it never returns object content
- if bulk content fetch is needed later, it should be designed as a separate API

### Pagination Semantics

Switch directly to token-based paging:

- request: `prefix`, `limit`, `page_token`
- response: `items`, `next_page_token`

Do not preserve the old `start_after` behavior.

## Request Flows

### Put Object

Suggested flow:

1. validate `key`, `size`, and `metadata`
2. if the object exists and `overwrite=false`, return conflict
3. generate a new `blob_id`
4. write to `BlobStore` first
5. write to `ObjectCatalog` second
6. if the catalog write fails, execute best-effort blob delete
7. if rollback fails, let `ObjectJanitor` clean it up

Reasoning:

- metadata is the final object visibility switch
- this makes it easier to ensure reads only see complete objects

### Head Object

Suggested flow:

1. query `ObjectRecord` from `ObjectCatalog`
2. return not found if absent
3. return summary only and do not read the blob

### Get Object

Suggested flow:

1. query `ObjectRecord` from `ObjectCatalog`
2. return not found if absent
3. verify the object state is `Ready`
4. read content from `BlobStore` through `blob_id`
5. return content with necessary metadata

### Delete Object

Suggested flow:

1. query the object from `ObjectCatalog`
2. reject delete when `ref_count > 0` or `pinned=true`
3. set the object state to `Deleting`
4. delete the blob
5. delete the catalog record
6. if blob deletion fails, keep `Deleting` and let `ObjectJanitor` retry

### Retain / Release

Object lifecycle APIs should be renamed to:

- `RetainObject`
- `ReleaseObject`

This is clearer than `AddObjectRef` / `RemoveObjectRef`.

Rules:

- `RetainObject`
  - `ref_count += 1`
- `ReleaseObject`
  - `ref_count -= 1`
  - when `ref_count == 0 && !pinned`, delete orchestration may begin

## Consistency And Failure Recovery

### Consistency Principles

This design intentionally avoids complex distributed transactions and uses a simple recoverable eventual consistency model:

- metadata is the only source of object visibility
- blob deletion failures are retriable
- object deletion must be idempotent
- write failures must be recoverable

### Orphan Blobs

Orphan blobs mainly come from two scenarios:

- blob write succeeds but catalog write fails
- overwrite succeeds but delayed deletion of the old blob fails

Handling strategy:

- do best-effort cleanup on the request path first
- let `ObjectJanitor` perform periodic cleanup in the background

### `Deleting` State

`Deleting` is the only intermediate state kept in phase one, and its purpose is very explicit:

- the object has entered delete orchestration
- it must no longer be exposed as a normal object
- background retries can proceed safely

## Configuration Design

Since breaking changes are accepted, the object storage configuration should move to a new explicit model instead of keeping the old mixed `storage` concept.

Suggested configuration:

```rust
pub struct ObjectStorageConfig {
    pub max_object_size: u64,
    pub metadata_backend: MetadataBackendConfig,
    pub blob_backend: BlobBackendConfig,
}
```

Where:

- `MetadataBackendConfig`
  - describes the catalog backend
- `BlobBackendConfig`
  - describes the blob backend

Suggested TOML:

```toml
[spearlet.object_storage]
max_object_size = 67108864

[spearlet.object_storage.metadata_backend]
backend = "rocksdb"
data_dir = "./data/spearlet/meta"
max_cache_size_mb = 512
compression_enabled = true

[spearlet.object_storage.blob_backend]
backend = "rustfs"
endpoint = "http://127.0.0.1:9000"
bucket = "spear-objects"
root = "prod/node-a"
connect_timeout_ms = 3000
read_timeout_ms = 10000
write_timeout_ms = 10000
create_bucket_if_missing = true
```

## RustFS Integration Design

### `RustFsBlobStore`

Provide a concrete `RustFsBlobStore` implementation for `BlobStore`.

Responsibilities:

- initialize the RustFS client
- manage bucket and root configuration
- implement `put/get/head/delete`
- map backend errors into a unified `ObjectError`

Requirements:

- requests must use timeouts
- delete must stay idempotent
- not found, transient errors, and permanent errors must be clearly distinguished

### Path Layout

The path strategy should not become a large abstraction.

If the only need is path generation, use a small concrete component such as:

- `BlobPathLayout`

Responsibilities:

- generate RustFS paths from `blob_id`
- centralize shard layout
- keep path construction out of service logic

## Suggested Module Organization

The object subsystem should live in a compact module tree and should not be split too finely.

Recommended structure:

- `src/spearlet/object/mod.rs`
- `src/spearlet/object/model.rs`
- `src/spearlet/object/service.rs`
- `src/spearlet/object/catalog.rs`
- `src/spearlet/object/blob.rs`
- `src/spearlet/object/janitor.rs`
- `src/spearlet/object/catalog_kv.rs`
- `src/spearlet/object/blob_rustfs.rs`

Notes:

- `catalog.rs`
  - trait and query / patch models
- `blob.rs`
  - trait and blob request / response models
- `service.rs`
  - business orchestration
- `janitor.rs`
  - background retry and cleanup
- `catalog_kv.rs`
  - `KvStore`-backed catalog implementation
- `blob_rustfs.rs`
  - RustFS backend implementation

## Assembly Strategy

There is no need for a separate `ObjectStoreFactory` abstraction.

If the system assembles the service once at startup, a clear builder function is sufficient:

```rust
pub async fn build_object_service(
    config: &ObjectStorageConfig,
) -> Result<ObjectService, ObjectError>
```

This is simpler and avoids abstraction for its own sake.

## Migration Guidance

### Phase 1: Define The New Model

- add `ObjectRecord`
- add `ObjectState`
- add `ObjectCatalog`
- add `BlobStore`

### Phase 2: Redesign API Semantics

- split `HeadObject` and `GetObject`
- make `ListObjects` return summaries only
- rename `AddObjectRef/RemoveObjectRef` to `RetainObject/ReleaseObject`
- introduce `page_token`

### Phase 3: Implement Catalog And Blob Backends

- implement `KvObjectCatalog`
- introduce the `BlobStore` abstraction with `LocalFsBlobStore` as the current compatibility implementation
- replace `LocalFsBlobStore` with `RustFsBlobStore`
- inject both into `ObjectService`

### Phase 4: Add Background Cleanup

- implement `ObjectJanitor`
- retry deletion of `Deleting` objects
- clean orphan blobs

### Phase 5: Remove The Old Model

- remove `StoredObject.value` persistence
- remove old object APIs
- remove old `storage` configuration mapping

## Current Implementation Status

At the current code state, SMS already includes:

- a `BlobStore` trait
- `LocalFsBlobStore` as the current local compatibility backend
- `RustFsBlobStore` backed by an S3-compatible RustFS client
- object handlers routed through `BlobStore` instead of embedding filesystem details directly
- runtime wiring that injects a shared `BlobStore` into gateway and web admin state
- a structured `object_storage` config with `local_fs` and `rustfs` sections
- an `sms-rustfs` Cargo feature so RustFS/AWS dependencies are compiled only into SMS-oriented builds; it is intentionally not a default feature while `sms`, `spearlet`, and `debug-server` still share one crate
- a local Docker Compose path that uses debug-profile container builds to keep RustFS-enabled local startup practical

That means the remaining RustFS work should focus on deployment hardening, credential management conventions, and end-to-end validation against a real RustFS cluster, rather than rewriting the object API handlers again.

## Testing Strategy

Testing should focus on semantic correctness rather than compatibility with the old implementation.

### Unit Tests

- `ObjectService`
  - rules for put/head/get/list/delete/retain/release/pin/unpin
- `KvObjectCatalog`
  - catalog encode/decode, pagination, and patch logic
- `RustFsBlobStore`
  - error mapping, timeout handling, and idempotent delete

### Integration Tests

- configuration assembly
  - verifies correct catalog and blob backend creation
- gRPC / HTTP APIs
  - verifies the new protocol semantics
- RustFS integration
  - verifies recovery when blob write succeeds but catalog write fails

### Failure Tests

- blob write succeeds but catalog write fails
- blob delete fails after the object enters `Deleting`
- `HeadObject` never triggers a blob read
- `ListObjects` never reads blob content

## Observability Requirements

Keep a small set of essential metrics:

- `object_put_total`
- `object_head_total`
- `object_get_total`
- `object_delete_total`
- `object_blob_delete_failures_total`
- `object_catalog_write_failures_total`
- `object_janitor_retry_total`
- `object_orphan_blob_cleanup_total`

Suggested log fields:

- `key`
- `blob_id`
- `size`
- `operation`
- `backend`
- `latency_ms`
- `result`

## Risks And Trade-Offs

### Benefits

- a clearer system spine
- more natural RustFS integration
- cleaner API semantics
- lower cost to support additional blob backends later
- code organization that fits Rust modularity better

### Costs

- the breaking change affects proto, HTTP APIs, and callers
- the old object model and configuration must be removed in one pass
- a background janitor is required for failure convergence

### Judgment

These costs are reasonable and much smaller than the long-term complexity of keeping the old model alive.

## Conclusion

The recommended redesign for the `spearlet` object subsystem is:

- `ObjectCatalog` for metadata
- `BlobStore` for blob content
- `ObjectService` for lifecycle orchestration
- `ObjectJanitor` for background cleanup
- `RustFsBlobStore` as the RustFS integration point

Compared with a compatibility-driven extension of the current object service, this design is clearer, more elegant, and better aligned with long-term maintainability.

If implementation starts next, the recommended priority is:

1. define the new data model and new APIs
2. implement `KvObjectCatalog` and `RustFsBlobStore`
3. implement `ObjectService` and `ObjectJanitor`
4. delete the old protocol and old storage model
