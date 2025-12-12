# Resource Service Code Refactoring Guide

## Overview
This document describes the refactoring of the `ResourceService` to eliminate code duplication in average calculation methods.

## Problem Identified
The `get_average_cpu_usage` and `get_average_memory_usage` functions contained duplicate code patterns:
- Both functions retrieved the resource list
- Both checked for empty resources and returned 0.0
- Both calculated totals by summing specific fields
- Both performed the same division: `total / resources.len() as f64`

## Solution Implemented
Created a generic helper function `calculate_average_field` that:
- Accepts a closure to extract the desired field from `NodeResourceInfo`
- Handles the common logic for empty resource lists
- Performs the average calculation using the extracted field values
- Returns the calculated average

## Code Changes

### Before Refactoring
```rust
pub async fn get_average_cpu_usage(&self) -> Result<f64, SmsError> {
    let resources = self.list_resources().await?;
    if resources.is_empty() {
        return Ok(0.0);
    }
    
    let total: f64 = resources.iter().map(|r| r.cpu_usage_percent).sum();
    Ok(total / resources.len() as f64)
}

pub async fn get_average_memory_usage(&self) -> Result<f64, SmsError> {
    let resources = self.list_resources().await?;
    if resources.is_empty() {
        return Ok(0.0);
    }
    
    let total: f64 = resources.iter().map(|r| r.memory_usage_percent).sum();
    Ok(total / resources.len() as f64)
}
```

### After Refactoring
```rust
async fn calculate_average_field<F>(&self, field_extractor: F) -> Result<f64, SmsError>
where
    F: Fn(&NodeResourceInfo) -> f64,
{
    let resources = self.list_resources().await?;
    if resources.is_empty() {
        return Ok(0.0);
    }
    
    let total: f64 = resources.iter().map(field_extractor).sum();
    Ok(total / resources.len() as f64)
}

pub async fn get_average_cpu_usage(&self) -> Result<f64, SmsError> {
    self.calculate_average_field(|r| r.cpu_usage_percent).await
}

pub async fn get_average_memory_usage(&self) -> Result<f64, SmsError> {
    self.calculate_average_field(|r| r.memory_usage_percent).await
}
```

## Benefits
1. **Reduced Code Duplication**: Eliminated repeated logic across multiple functions
2. **Improved Maintainability**: Changes to average calculation logic only need to be made in one place
3. **Enhanced Extensibility**: Easy to add new average calculation methods for other fields
4. **Better Testability**: Common logic is centralized and easier to test

## Testing
All existing tests continue to pass, confirming that the refactoring maintains the same functionality while improving code quality.

## Files Modified
- `src/services/resource.rs`: Refactored average calculation methods

## Future Enhancements
The `calculate_average_field` helper function can be used to easily add average calculations for other resource metrics such as:
- Average disk usage
- Average network throughput
- Average load averages

## Related Files
- `src/services/resource.rs`: Main implementation
- Tests in the same file verify functionality