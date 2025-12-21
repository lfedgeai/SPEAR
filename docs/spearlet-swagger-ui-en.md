# SPEARlet Swagger UI Implementation

## Overview

This document describes the implementation and usage of Swagger UI for the SPEARlet HTTP gateway, providing interactive API documentation similar to the SMS module.

## Implementation Details

### Added Components

1. **Enhanced API Documentation Function** (`api_docs`)
   - Comprehensive OpenAPI 3.0.0 specification
   - Detailed descriptions for all API endpoints
   - Support for multiple languages (English/Chinese)
   - Proper HTTP status codes and response schemas

2. **Swagger UI HTML Page** (`swagger_ui`)
   - Interactive web interface for API exploration
   - CDN-based Swagger UI resources (v5.9.0)
   - Custom styling and branding
   - Responsive design for different screen sizes

3. **New HTTP Routes**
   - `/api/openapi.json` - OpenAPI specification endpoint
   - `/swagger-ui` - Main Swagger UI interface
   - `/docs` - Alternative access to Swagger UI

### Configuration

The Swagger UI feature is controlled by the `swagger_enabled` configuration option in the HTTP settings:

```toml
[http]
swagger_enabled = true
```

When enabled, the following routes become available:
- Original `/api-docs` route (JSON format)
- New `/api/openapi.json` route (enhanced OpenAPI spec)
- New `/swagger-ui` and `/docs` routes (interactive UI)

## API Endpoints Documented

The Swagger UI includes comprehensive documentation for:

### Health & Status
- `GET /health` - Basic health check
- `GET /status` - Detailed node status information

### Object Storage
- `PUT /objects/{key}` - Store object data
- `GET /objects/{key}` - Retrieve object data
- `DELETE /objects/{key}` - Delete object data
- `GET /objects` - List all objects

### Reference Management
- `POST /objects/{key}/refs` - Add object reference
- `DELETE /objects/{key}/refs` - Remove object reference

### Pinning Operations
- `POST /objects/{key}/pin` - Pin object to prevent cleanup
- `DELETE /objects/{key}/pin` - Unpin object

### Function Execution
- `POST /functions/invoke` - Invoke a function with specified parameters
- `GET /functions/executions/{execution_id}/status` - Get execution status
- `POST /functions/executions/{execution_id}/cancel` - Cancel function execution
- `GET /functions/stream` - Stream function execution results

### Task Management
- `GET /tasks` - List all tasks with optional filtering
- `GET /tasks/{task_id}` - Get detailed task information
- `DELETE /tasks/{task_id}` - Delete a specific task
- `GET /tasks/{task_id}/executions` - Get execution history for a task

### Monitoring & Statistics
- `GET /functions/health` - Get function service health status
- `GET /functions/stats` - Get comprehensive function service statistics

## Usage Instructions

### Accessing Swagger UI

1. **Start SPEARlet** with Swagger enabled:
   ```bash
   cargo run --bin spearlet
   ```

2. **Open Swagger UI** in your browser:
   - Primary URL: `http://localhost:8081/swagger-ui`
   - Alternative URL: `http://localhost:8081/docs`

3. **Access OpenAPI Specification**:
   - JSON format: `http://localhost:8081/api/openapi.json`
   - Legacy format: `http://localhost:8081/api-docs`

### Using the Interface

1. **Browse API Endpoints**: All endpoints are organized by tags (health, objects, references, pinning)

2. **Try API Calls**: Click "Try it out" on any endpoint to:
   - Fill in required parameters
   - Execute requests directly from the browser
   - View response data and status codes

3. **View Schemas**: Expand response schemas to understand data structures

4. **Download Specification**: Use the OpenAPI JSON URL to import into other tools

## Technical Implementation

### File Modifications

- **`spearlet/http_gateway.rs`**:
  - Added `Html` and `IntoResponse` imports
  - Enhanced `api_docs()` function with comprehensive OpenAPI spec
  - Added `swagger_ui()` function returning HTML page
  - Updated route configuration to include new endpoints

### Dependencies

The implementation uses:
- **Axum** for HTTP routing and responses
- **Swagger UI** (v5.9.0) via CDN for the interactive interface
- **OpenAPI 3.0.0** specification format

### Security Considerations

- All resources loaded from trusted CDN (unpkg.com)
- No sensitive information exposed in API documentation
- Swagger UI can be disabled via configuration

## Comparison with SMS Module

The SPEARlet Swagger UI implementation follows the same pattern as the SMS module:

| Feature | SMS | SPEARlet |
|---------|-----|----------|
| OpenAPI Spec | ✅ | ✅ |
| Interactive UI | ✅ | ✅ |
| Multiple Routes | ✅ | ✅ |
| Configuration Control | ✅ | ✅ |
| Bilingual Support | ✅ | ✅ |

## Future Enhancements

Potential improvements for the Swagger UI implementation:

1. **Custom Themes**: Add SPEARlet-specific branding and colors
2. **Authentication**: Integrate with authentication mechanisms if added
3. **Examples**: Add more comprehensive request/response examples
4. **Validation**: Include request validation schemas
5. **Offline Mode**: Bundle Swagger UI resources locally

## Troubleshooting

### Common Issues

1. **Swagger UI not loading**:
   - Verify `swagger_enabled = true` in configuration
   - Check that SPEARlet is running on the correct port
   - Ensure network connectivity to CDN resources

2. **API calls failing**:
   - Verify SPEARlet gRPC server is running
   - Check HTTP gateway connection status
   - Review server logs for error messages

3. **Missing endpoints**:
   - Ensure you're using the correct URL (`/swagger-ui` or `/docs`)
   - Refresh the page to reload the OpenAPI specification
   - Check browser console for JavaScript errors

### Logs and Debugging

Monitor SPEARlet logs for:
- HTTP gateway startup messages
- gRPC connection status
- Request processing errors
- Configuration loading issues

## Conclusion

The SPEARlet Swagger UI implementation provides a comprehensive, interactive API documentation system that matches the functionality available in the SMS module. Users can now easily explore and test SPEARlet APIs through a modern web interface.