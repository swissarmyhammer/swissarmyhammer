update the documentation and make sure it is up to date. use the review/documentation prompt

## Proposed Solution

I've completed a comprehensive documentation review following the review/documentation prompt process. The analysis revealed that SwissArmyHammer has excellent documentation infrastructure and comprehensive coverage, but requires updates to fix some accuracy issues.

### Key Actions Taken:
1. Reviewed all documentation files in the `./doc` folder structure
2. Cross-referenced documentation against actual source code and CLI behavior  
3. Verified API documentation accuracy in `lib.rs` and core modules
4. Identified discrepancies between documented and actual built-in prompts
5. Created comprehensive `DOCUMENTATION_REVIEW.md` with findings and recommendations

### Main Issues Identified:
- Built-in prompts documentation lists fictional examples instead of actual prompts
- Some installation command examples may be outdated
- Minor gaps in API documentation coverage

### Implementation Plan:
1. Fix `doc/src/builtin-prompts.md` to accurately reflect actual built-in prompts
2. Verify and update installation instructions  
3. Test all code examples for accuracy
4. Complete any missing rustdoc coverage
5. Rebuild and verify all documentation links

The documentation foundation is excellent - these targeted updates will ensure complete accuracy and eliminate user confusion.