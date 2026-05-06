---
assignees:
- claude-code
position_column: todo
position_ordinal: e180
project: keyboard-navigation
title: Sneak code generator in swissarmyhammer-focus (pure Rust + Tauri command)
---
## What

Pure algorithm that produces short, prefix-free key codes for the Jump-To overlay — vim-sneak / jumpy / AceJump labels. Lives in **`swissarmyhammer-focus`** as Rust; exposed to the React frontend via a Tauri command in `kanban-app/src/commands.rs`.

Architectural fit: spatial-nav vocabulary is Rust-authoritative in this workspace (`Direction`, `FullyQualifiedMoniker`, etc. are defined in Rust and mirrored in TypeScript). The sneak code algorithm is generic spatial-nav infrastructure with no domain or UI knowledge — putting it in `swissarmyhammer-focus` keeps the layering consistent and means any future consumer (mirdan-app) gets it via dep, not by copy-pasting TypeScript.

### Steps

1. **Create `swissarmyhammer-focus/src/sneak.rs`** with:

   ```rust
   /// Ergonomic alphabet, ordered by priority — home row first, then top
   /// row, then bottom row. Skips letters with high visual confusion
   /// (`i`/`1`/`l`, `o`/`0`).
   pub const SNEAK_ALPHABET: &[char] = &[
       'a','s','d','f','j','k','g','h','w','e','r','u',
       'p','q','t','y','z','x','c','v','n','m','b',
   ]; // pick a final 23-letter list; document the choices

   /// Generate `count` distinct, prefix-free key codes drawn from
   /// [`SNEAK_ALPHABET`]. Codes are ordered by ergonomic priority
   /// (shortest + easiest first). Returned codes are lowercase strings.
   ///
   /// # Errors
   ///
   /// Returns an error when `count > SNEAK_ALPHABET.len().pow(2)` —
   /// today the cap is around 529, well above any realistic Jump-To
   /// target count. Hitting it indicates an upstream bug.
   pub fn generate_sneak_codes(count: usize) -> Result<Vec<String>, SneakError>;

   #[derive(Debug, thiserror::Error)]
   pub enum SneakError {
       #[error("too many jump targets: {0} exceeds capacity {1}")]
       TooManyTargets(usize, usize),
   }
   ```

   Algorithm:
   - `count == 0` → empty vec.
   - If `count <= alphabet.len()` → single-letter codes from the front of the alphabet.
   - Else split codes into two-letter prefixes. Reserve the last `K` letters of the alphabet as "two-letter prefixes": for each prefix letter `X`, codes `Xa`, `Xs`, `Xd`, ... become valid two-letter codes. Single-letter codes use the first `alphabet.len() - K` letters; two-letter codes use combinations starting with the last `K` letters. Choose `K` to fit `count` codes (smallest `K` such that `(alphabet.len() - K) + K * alphabet.len() >= count`).
   - Prefix-free by construction: a single-letter code never starts a two-letter code (different prefix bucket).
   - Pure logic; no I/O, no `unsafe`, no dependencies beyond `thiserror`.

2. **Expose via lib.rs**: `pub mod sneak; pub use sneak::{generate_sneak_codes, SNEAK_ALPHABET, SneakError};`

3. **Tauri command in `kanban-app/src/commands.rs`**:

   ```rust
   #[tauri::command]
   pub fn generate_jump_codes(count: usize) -> Result<Vec<String>, String> {
       swissarmyhammer_focus::generate_sneak_codes(count)
           .map_err(|e| e.to_string())
   }
   ```

   Register it in the Tauri builder alongside the other commands. Read `kanban-app/src/commands.rs:173+` for the existing `#[tauri::command]` registration pattern and the warning banner at the top of the file about state-mutating commands. Sneak code generation is pure (no state mutation), so it's a clean addition.

4. **Frontend bindings**: in `kanban-app/ui/src/lib/sneak-codes.ts` (a thin wrapper, not the algorithm itself):

   ```ts
   import { invoke } from "@tauri-apps/api/core";

   /** Generate `count` distinct prefix-free Jump-To codes via the Rust kernel. */
   export async function generateSneakCodes(count: number): Promise<string[]> {
       return await invoke<string[]>("generate_jump_codes", { count });
   }
   ```

   The JumpToOverlay component (next task) calls this once on open. The 30-keystroke-per-second user experience is far below any noticeable invoke latency.

## Acceptance Criteria

- [ ] `swissarmyhammer-focus::generate_sneak_codes(0)` returns an empty vec.
- [ ] `swissarmyhammer-focus::generate_sneak_codes(N)` returns N distinct strings for `1 <= N <= alphabet.len().pow(2)`.
- [ ] No element of the returned vec is a prefix of any other element.
- [ ] Codes for small N use the home-row letters first.
- [ ] `generate_sneak_codes(N+1)` returns `Err(SneakError::TooManyTargets(...))` when `N` is the maximum capacity.
- [ ] Tauri command `generate_jump_codes` is registered and round-trips correctly through `invoke`.
- [ ] Frontend `generateSneakCodes(count)` resolves to the same vec the Rust impl produces.

## Tests

- [ ] New Rust unit tests in `swissarmyhammer-focus/src/sneak.rs` (`#[cfg(test)] mod tests`):
  - `generates_empty_for_zero_count` — `generate_sneak_codes(0)` is empty.
  - `generates_distinct_codes` — for N in `[1, 5, 10, 23, 50, 200, 500]`, every pair is distinct.
  - `prefix_free_invariant` — for the same range of N, no code is a prefix of any other (brute-force check: for each pair, neither `starts_with` the other).
  - `single_letter_codes_use_home_row_first` — for N=4, returned codes match the first 4 letters of `SNEAK_ALPHABET`.
  - `errors_when_count_exceeds_capacity` — `generate_sneak_codes(SNEAK_ALPHABET.len().pow(2) + 1)` returns `Err(TooManyTargets(...))`.
- [ ] Tauri command test in `kanban-app/tests/`: round-trip `generate_jump_codes` with a known count and assert the result matches the Rust impl.
- [ ] Frontend test `kanban-app/ui/src/lib/sneak-codes.test.ts`: mock `invoke`, assert the wrapper passes `count` through and returns the resolved array.
- [ ] Test command: `cargo nextest run -p swissarmyhammer-focus && cd kanban-app/ui && pnpm test sneak-codes` — passes.

## Workflow

- Use `/tdd` — write the Rust unit tests first (will fail because `sneak::generate_sneak_codes` doesn't exist); implement; re-run. Then add the Tauri command and the TS wrapper.