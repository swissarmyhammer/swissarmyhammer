Do no 'find' the branch to merge with git merge-base and then merge -- just merge already with git merge-base. The current code has needless steps.

Do not fall back to 'main' when merging -- just use merge-base for merg

find_merge_target_branch does not need to exist - the merge issue tool needs to directly call a method in the git.rs to merge-base

get_stored_source_branch_fallback does not need to exist


create_work_branch_with_source is a bad idea -- specifically passing the source is a bad idea -- particularly when you are always passing None in real cases. just make this a create_work_branch and get rid of source_branch: Option<&str>,


Do not do this

```
                        let target_branch = ops
                            .find_merge_target_branch(&issue_name)
                            .unwrap_or_else(|_| "main".to_string());
```
merge_issue_branch_auto should just return the target branch we just merged to -- checking again is wasteful


when we get to tracing::error!("Merge failed for issue '{}': {}", issue_name, e);

we're doing a bunch of nonsense trying to parse up the merge error -- we just need to take the error message and use the abort tool, then return the error