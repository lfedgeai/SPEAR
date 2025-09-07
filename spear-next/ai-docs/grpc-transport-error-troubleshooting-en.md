# gRPC Transport Error Troubleshooting Guide

## Problem Description

When running the SPEAR Node Service, you may encounter the following error:

```
ERROR sms: gRPC server error: transport error
```

## Common Causes and Solutions

### 1. Port Already in Use

**Symptoms:**
- Transport error appears immediately after application startup
- Service cannot bind to specified port

**Diagnosis:**
```bash
# Check if port 50051 is in use
lsof -i :50051

# Check if port 8080 is in use  
lsof -i :8080
```

**Solution:**
```bash
# Kill the process using the port
kill <PID>

# Or modify configuration to use different ports
# In config.toml:
[sms]
grpc_addr = "0.0.0.0:50052"  # Use different port
http_addr = "0.0.0.0:8081"   # Use different port
```

### 2. Network Configuration Issues

**Symptoms:**
- gRPC server starts but cannot accept connections
- HTTP gateway cannot connect to gRPC service

**Solution:**
```bash
# Check firewall settings
sudo ufw status

# Ensure ports are not blocked by firewall
sudo ufw allow 50051
sudo ufw allow 8080
```

### 3. Service Startup Order Issues

**Symptoms:**
- HTTP gateway tries to connect before gRPC server is fully started
- Connection retry messages appear

**Normal Behavior:**
```
WARN sms: Failed to connect to Node gRPC service: transport error, retrying in 2s...
INFO sms: Connected to Node gRPC service at http://0.0.0.0:50051
```

This is normal startup behavior - the HTTP gateway will automatically retry connections.

### 4. Configuration Errors

**Symptoms:**
- Service cannot parse configured addresses
- Bind address format errors

**Check Configuration:**
```toml
[sms]
grpc_addr = "0.0.0.0:50051"  # Correct format
http_addr = "0.0.0.0:8080"   # Correct format

# Incorrect examples:
# grpc_addr = "50051"        # Missing IP address
# grpc_addr = ":50051"       # Incomplete format
```

## Verifying Service Status

### 1. Check if Services are Running

```bash
# Check port listening status
netstat -an | grep -E "(50051|8080)"

# Should see output like:
# tcp4  0  0  *.50051  *.*  LISTEN
# tcp4  0  0  *.8080   *.*  LISTEN
```

### 2. Test HTTP Gateway

```bash
# Health check
curl http://localhost:8080/health

# Expected response:
# {"service":"sms","status":"healthy","timestamp":"..."}
```

### 3. Test gRPC Service

```bash
# Test with grpcurl (if installed)
grpcurl -plaintext localhost:50051 list

# Or check for success messages in service logs:
# INFO sms: Registered services: NodeService, TaskService
```

## Log Analysis

### Normal Startup Logs

```
INFO sms: Starting Node Service (Spear Node Management Service)...
INFO sms: Configuration loaded:
INFO sms:   gRPC server: 0.0.0.0:50051
INFO sms:   HTTP gateway: 0.0.0.0:8080
INFO sms: Starting HTTP gateway on 0.0.0.0:8080
INFO sms: Starting gRPC server on 0.0.0.0:50051
INFO sms: Registered services: NodeService, TaskService
WARN sms: Failed to connect to Node gRPC service: transport error, retrying in 2s...
INFO sms: Connected to Node gRPC service at http://0.0.0.0:50051
INFO sms: Connected to Task gRPC service at http://0.0.0.0:50051
INFO sms: HTTP gateway listening on 0.0.0.0:8080
INFO sms: Swagger UI available at: http://0.0.0.0:8080/swagger-ui/
```

### Problem Log Patterns

```
ERROR sms: gRPC server error: transport error
INFO sms: Node service stopped
```

If you see this pattern, it means the gRPC server cannot start, usually due to port conflicts.

## Prevention Measures

1. **Use Configuration Files**: Specify ports through configuration files to avoid hardcoding
2. **Port Checking**: Check port availability before startup
3. **Graceful Shutdown**: Use Ctrl+C to properly shut down services, avoiding port residue
4. **Monitor Logs**: Pay attention to startup logs to identify issues early

## Related Documentation

- [CLI Configuration Guide](cli-configuration-en.md)
- [Service Configuration](../config.toml)
- [HTTP API Documentation](http-api-en.md)