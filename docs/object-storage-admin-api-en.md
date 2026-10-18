# Object Storage Admin API

This document describes the current admin-side object APIs exposed by SMS Web Admin.

These APIs are the first implementation phase of the object-domain frontend redesign. Their goals are:

- let the frontend operate around the `Objects` domain
- provide admin APIs that are separate from the old `files` page semantics
- reuse the current file storage through a compatibility layer until the new object backend is fully implemented

## Design Position

This API set is an "object admin compatibility layer", not the final object service protocol.

Current characteristics:

- paths use `/admin/api/objects*`
- response shapes already match the frontend object domain
- the underlying storage still reuses the current file directory
- `key` is currently the compatibility file ID
- `state/ref_count/pinned` currently return compatibility defaults

## Routes

### List Objects

- `GET /admin/api/objects`

Query parameters:

- `prefix`
- `limit`
- `page_token`

Example response:

```json
{
  "items": [
    {
      "key": "8d4b8f8d-0f33-4f82-a8e0-5b4e0f8b6f1d",
      "name": "tool.wasm",
      "size": 12345,
      "updated_at": 1756656000,
      "state": "ready",
      "ref_count": 1,
      "pinned": false
    }
  ],
  "total_count": 1,
  "next_page_token": null
}
```

### Get Object Metadata

- `GET /admin/api/objects/{key}`

Example response:

```json
{
  "found": true,
  "key": "8d4b8f8d-0f33-4f82-a8e0-5b4e0f8b6f1d",
  "size": 12345,
  "state": "ready",
  "ref_count": 1,
  "pinned": false,
  "name": "tool.wasm",
  "created_at": 1756656000,
  "updated_at": 1756656000
}
```

### Download Object Content

- `GET /admin/api/objects/{key}/content`

Returns the binary object content.

### Upload Object

- `POST /admin/api/objects`

Request body:

- upload raw binary content
- optional headers:
  - `Content-Type`
  - `x-file-name`

Example response:

```json
{
  "success": true,
  "id": "8d4b8f8d-0f33-4f82-a8e0-5b4e0f8b6f1d",
  "key": "8d4b8f8d-0f33-4f82-a8e0-5b4e0f8b6f1d",
  "name": "tool.wasm",
  "uri": "object://8d4b8f8d-0f33-4f82-a8e0-5b4e0f8b6f1d"
}
```

### Delete Object

- `DELETE /admin/api/objects/{key}`

Example response:

```json
{
  "success": true
}
```

Delete rules:

- deletion is rejected when `ref_count > 0`
- deletion is rejected when `pinned = true`

### Retain Object

- `POST /admin/api/objects/{key}/retain`

Example response:

```json
{
  "success": true,
  "key": "8d4b8f8d-0f33-4f82-a8e0-5b4e0f8b6f1d",
  "ref_count": 1,
  "pinned": false,
  "state": "ready"
}
```

### Release Object

- `POST /admin/api/objects/{key}/release`

It returns a failure message when the reference count is already `0`.

### Pin Object

- `POST /admin/api/objects/{key}/pin`

### Unpin Object

- `POST /admin/api/objects/{key}/unpin`

### Get Object Storage Status

- `GET /admin/api/object-storage/status`

Example response:

```json
{
  "backend": "local-fs-blobstore",
  "healthy": true,
  "message": "Using LocalFsBlobStore compatibility backend rooted at ./data/objects"
}
```

## Current Boundary

The current admin API boundary has been reduced to:

- `/admin/api/objects*` is the new frontend object-domain API
- the underlying implementation is now routed through `BlobStore`, supporting both `LocalFsBlobStore` and `RustFsBlobStore`

This means the system is now "protocol-objectized behind a stable BlobStore boundary", so switching between local compatibility storage and RustFS no longer requires rewriting the object API handlers.

## Current Limitations

The current implementation still has the following limitations:

- `key` is not yet the final business object key and is still a compatibility ID
- `retain/release/pin/unpin` already persist into the compatibility sidecar, but this is still not the final `ObjectCatalog`
- the task creation page already submits a structured object reference `{ "kind": "object", "object_key": "<key>" }`

## Next Steps

Recommended next phase:

1. add a real object catalog backend in SMS or SPEARlet
2. decouple object `key` from the compatibility file ID
3. add real APIs for `retain/release/pin/unpin`
4. hook task lifecycle into retain/release now that executable references are structured

## Related Documents

- [object-storage-frontend-design-en.md](./object-storage-frontend-design-en.md)
- [rustfs-object-storage-design-en.md](./rustfs-object-storage-design-en.md)
- [web-admin-ui-guide-en.md](./web-admin-ui-guide-en.md)
