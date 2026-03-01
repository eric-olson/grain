# Rust Style Guide

Conventions for this codebase. Follow these when writing new code or modifying existing code.

## Formatting & Linting

```bash
cargo fmt            # Format all code
cargo clippy         # Lint — treat warnings as errors
```

Run both before committing.

## Error Handling

- Use `thiserror` for typed error enums in library-facing modules (e.g., `FileError`, `ParseHexError`). Each module that can fail defines its own error type.
- Use `eprintln!` for errors in UI-facing code that don't propagate (e.g., file open failures in `app.rs`).
- Convert errors to `String` with `.to_string()` only at the UI boundary, not in library code.
- Never use `Result<T, String>` — always define a proper error enum.

## Threading & Background Tasks

- Spawn background work with `std::thread::spawn`, not async/tokio.
- Use `std::sync::mpsc::channel` to send results back from background threads. The background thread sends a single message with the final result; the UI thread polls with `try_recv()`.
- Group the `Receiver<T>` into a state struct (e.g., `SearchState`, `StrideDetectState`) with a `poll()` method that wraps `try_recv()`.
- The UI thread calls `poll()` each frame and calls `ctx.request_repaint()` while tasks are active.
- Clone data into `Arc<Vec<u8>>` before spawning — never hold references across thread boundaries.
- Use `parking_lot::Mutex` (not `std::sync::Mutex`) when shared mutable state is needed. It doesn't poison and doesn't require `.unwrap()` on lock.

## Imports

- All imports at the top of the file. Group in this order, separated by blank lines:
  1. `std::` imports
  2. External crate imports
  3. `crate::` imports
- Use `std::fmt` style for module-level imports when accessing both the module and its members.
- No inline imports inside function bodies.

## Struct Design

- Use `Default` impl for structs with many fields (see `App`, `PixelGridViewer`).
- Group related fields into sub-structs (e.g., `SearchPanel`, `StrideDetect`) to reduce top-level field count and clarify ownership.
- Public fields on data-transfer structs (`CursorInfo`, `StrideCandidate`, `SearchMatch`). No getters needed.
- Private fields on stateful structs with methods (`App`, `PixelGridViewer`, `MappedFile`).
- Derive `Clone, Debug` on small data structs. Add `Copy, PartialEq, Eq` where appropriate (e.g., enums like `DisplayMode`).

## Function Signatures

- Prefer `&[T]` / `&mut [T]` over `&Vec<T>` / `&mut Vec<T>` in function parameters.
- Don't write trivial wrapper functions — call the underlying method directly (e.g., use `.reverse_bits()` instead of a `reverse_bits()` wrapper).

## Naming

- Modules: `snake_case` matching the concept (`stride_detect`, `sync_search`, `file_handler`).
- Types: `PascalCase`. Suffix state-holding types with `State` (`SearchState`, `StrideDetectState`).
- Background launcher functions: `verb_background` (e.g., `search_background`, `detect_stride_background`).
- Internal helpers: private `fn`, no `pub` unless needed by another module.

## Module Organization

- One concept per module. Keep modules focused (viewer, search, stride detection are separate).
- `pub` only what `app.rs` needs. Internal helpers stay private.

## Unsafe Code

- Add a `// SAFETY:` comment on every `unsafe` block explaining why the operation is sound.

## egui / eframe Patterns

- All UI layout lives in `App::update()` via egui panels (`TopBottomPanel`, `SidePanel`, `CentralPanel`).
- Use `ui.input(|i| ...)` closures to read input state — don't hold input locks across frames.
- Cache textures in structs; invalidate explicitly via an `invalidate()` method when state changes rather than rebuilding every frame.
- Use `TextureOptions::NEAREST` for pixel-art style rendering.
- Use `egui::DragValue` for numeric inputs, not text fields.
- Avoid cloning large collections per-frame. Pass `&self.field` into scroll areas instead of cloning.

## Performance

- Memory-mapped I/O for file access — never read entire files into memory unless sharing with a background thread.
- Viewport-only rendering: compute and draw only the visible rows.
- Avoid allocations in hot loops. Pre-allocate with `Vec::with_capacity` when the size is known.
- Use `saturating_sub`, `saturating_add` for offset arithmetic to avoid underflow panics.
- Mark tight-loop helpers `#[inline]` (e.g., `shifted_byte`).

## Comments & Documentation

- Doc comments (`///`) on public functions and types that aren't self-explanatory.
- Inline comments for non-obvious math or bit manipulation (e.g., z-score calculation, MSB-first bit indexing).
- No boilerplate doc comments on trivial methods like `len()` or `name()`.

## Numeric Types

- `usize` for offsets, indices, and sizes throughout.
- Cast to `f64` for statistical calculations (z-scores, ratios), to `f32` for egui coordinates.
- Use `as isize` for signed offset arithmetic (scroll deltas), then clamp back to `usize`.
- Clamp zoom/stride values at the point of change, not at every use site.

## Display Trait

- Implement `std::fmt::Display` on enums that appear in the UI (e.g., `Variation`).
- Use `format!` with the Display impl rather than match arms at every call site.
