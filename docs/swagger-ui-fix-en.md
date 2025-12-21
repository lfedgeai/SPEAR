# Swagger UI API Path Fix Guide

## Problem Description

When clicking API endpoints (such as `nodes`) in Swagger UI, API calls were failing. After diagnosis, the root cause was identified as a mismatch between the paths defined in the OpenAPI specification and the actual HTTP routes.

## Problem Analysis

### Original Issue
- **OpenAPI Specification Paths**: `/nodes`, `/nodes/{uuid}`, `/resources`, etc.
- **Actual HTTP Routes**: `/api/v1/nodes`, `/api/v1/nodes/{uuid}`, `/api/v1/resources`, etc.
- **Result**: Swagger UI attempted to call incorrect paths, resulting in 404 errors

### Diagnosis Process
1. Confirmed service was running normally with both HTTP gateway and gRPC server started
2. Verified that actual API endpoint `/api/v1/nodes` was working correctly
3. Checked that Swagger UI page was accessible
4. Discovered that OpenAPI specification path definitions were missing the `/api/v1` prefix

## Fix Solution

### Fixed Files
- `src/http/handlers/docs.rs` - OpenAPI specification definition file

### Fixed Paths
1. **Node Management Paths**:
   - `/nodes` → `/api/v1/nodes`
   - `/nodes/{uuid}` → `/api/v1/nodes/{uuid}`
   - `/nodes/{uuid}/heartbeat` → `/api/v1/nodes/{uuid}/heartbeat`

2. **Resource Management Paths**:
   - `/nodes/{uuid}/resource` → `/api/v1/nodes/{uuid}/resource`
   - `/resources` → `/api/v1/resources`
   - `/nodes/{uuid}/with-resource` → `/api/v1/nodes/{uuid}/with-resource`

3. **Task Management Paths**:
   - Added `/api/v1/tasks` (POST, GET)
   - Added `/api/v1/tasks/{task_id}` (GET, DELETE)

### Fix Example

```rust
// Before fix
"/nodes": {
    "get": {
        "summary": "List all nodes",
        // ...
    }
}

// After fix
"/api/v1/nodes": {
    "get": {
        "summary": "List all nodes",
        // ...
    }
}
```

## Verification Steps

### 1. Restart Service
```bash
cargo run --bin sms
```

### 2. Verify OpenAPI Specification
```bash
curl -s http://localhost:8080/api/openapi.json | jq '.paths | keys'
```

### 3. Test API Endpoints
```bash
# Get node list
curl http://localhost:8080/api/v1/nodes

# Register new node
curl -X POST http://localhost:8080/api/v1/nodes \
  -H "Content-Type: application/json" \
  -d '{
    "ip_address": "192.168.1.100",
    "port": 8080,
    "region": "us-west-1",
    "zone": "us-west-1a",
    "instance_type": "t3.medium",
    "metadata": {
      "environment": "test",
      "version": "1.0.0"
    }
  }'
```

### 4. Verify Swagger UI
Visit `http://localhost:8080/swagger-ui/` and test all API endpoints.

## Fix Results

After the fix:
- ✅ All API endpoints in Swagger UI can be called normally
- ✅ OpenAPI specification matches actual routes completely
- ✅ Added task management related API definitions
- ✅ All paths include the correct `/api/v1` prefix

## Best Practices

1. **Maintain Consistency**: Ensure OpenAPI specification paths exactly match actual HTTP routes
2. **Version Control**: Use `/api/v1` prefix for API version control
3. **Completeness**: Ensure all available API endpoints are defined in the OpenAPI specification
4. **Test Verification**: After modifying OpenAPI specification, always restart service and perform complete testing

## Related Documentation

- [API Usage Guide (Chinese)](./api-usage-guide-zh.md)
- [API Usage Guide (English)](./api-usage-guide-en.md)
- [gRPC Transport Error Troubleshooting Guide](./grpc-transport-error-troubleshooting-zh.md)