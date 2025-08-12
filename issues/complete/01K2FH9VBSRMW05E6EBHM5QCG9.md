---
source_branch: "main"
completed: false
---

Get rid of these as yaml front matter -

- completed isn't 'in' the file -- it is being moved to completed
- file_path isn't 'in' the file -- it is from where it was loaded
- created_at isn't 'in' the file -- it is from the create date of the file. it also does not matter at all

We just DO NOT need yaml front matter in issues to make them work.

file path does not belong 'in' the yamls
    /// Whether the issue is completed
    pub completed: bool,
    /// The file path of the issue
    pub file_path: PathBuf,
    /// When the issue was created
    pub created_at: DateTime<Utc>,

## Proposed Solution

Based on my analysis, the Issue struct currently has three fields that are stored in YAML front matter but should be derived from the file system:

1. **completed**: Should be determined by whether the file is in `issues/` (false) or `issues/complete/` (true)
2. **file_path**: Should be derived from where the file was loaded from
3. **created_at**: Should be extracted from the file's creation/modification time

### Implementation Steps:

1. **Remove YAML front matter parsing/generation** from `filesystem.rs`
   - Remove the YAML parsing logic in `parse_issue_from_path`  
   - Remove YAML front matter generation in `update_issue` and `create_issue`

2. **Update Issue struct** to remove the three problematic fields
   - Keep `name` and `content` as core data
   - Remove `completed`, `file_path`, `created_at`

3. **Create methods to derive the removed fields**:
   - `Issue::is_completed(&self, file_path: &Path) -> bool` - check directory location
   - `Issue::get_file_path(&self, base_dir: &Path) -> PathBuf` - construct from name and base dir
   - `Issue::get_created_at(file_path: &Path) -> DateTime<Utc>` - get from file metadata

4. **Update all usage sites** to use the derived values instead of struct fields

5. **Update tests** to work without YAML front matter

This will make issues pure markdown files without any metadata pollution, and all metadata will be derived from the file system state.