# Rust Development Skill

## When to Use
- Adding Tauri commands
- Modifying backend logic
- Working with state
- Adding new modules

## Process

1. **Plan module location**: Choose appropriate directory
2. **Define types**: Create DTOs in `config/dto.rs` if needed
3. **Implement command**: Use `#[tauri::command]` attribute
4. **Register command**: Add to `lib.rs` invoke_handler
5. **Add error handling**: Use `Result<T, String>` return type

## Patterns

### Command Structure
```rust
#[tauri::command]
async fn my_command(
    param: String,
    state: State<'_, AppState>,
) -> Result<MyResponse, String> {
    let mut state = state.lock().await;
    // Implementation
    Ok(response)
}
```

### Error Handling
```rust
// Use thiserror for custom errors
#[derive(Debug, thiserror::Error)]
pub enum MyError {
    #[error("Operation failed: {0}")]
    OperationFailed(String),
}
```

### State Access
```rust
// Async state
let mut state = state.lock().await;

// Sync state (if needed)
let state = state.lock().map_err(|e| e.to_string())?;
```

## Commands
```bash
# Check compilation
cargo check --manifest-path src-tauri/Cargo.toml

# Run with warnings
cargo clippy --manifest-path src-tauri/Cargo.toml

# Run tests
cargo test --manifest-path src-tauri/Cargo.toml
```
