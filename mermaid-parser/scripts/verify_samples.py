#!/usr/bin/env python3
"""
Simple verification script to check extracted Mermaid samples.
"""

import os
from pathlib import Path
import random

def check_sample_structure(file_path):
    """Check if a sample file has proper structure."""
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            content = f.read()
        
        # Check for header comments
        has_source = 'Source:' in content
        has_type = 'Type:' in content
        
        # Check for actual mermaid content (non-empty after comments)
        lines = content.split('\n')
        non_comment_lines = [line for line in lines if not line.strip().startswith('//') and line.strip()]
        has_content = len(non_comment_lines) > 0
        
        return {
            'has_source': has_source,
            'has_type': has_type,
            'has_content': has_content,
            'line_count': len(non_comment_lines),
            'valid': has_source and has_type and has_content
        }
    except Exception as e:
        return {
            'error': str(e),
            'valid': False
        }

def main():
    """Verify sample files."""
    print("Verifying extracted Mermaid samples...")
    
    sample_dir = Path("mermaid-samples")
    if not sample_dir.exists():
        print("Error: mermaid-samples directory not found")
        return
    
    # Get all .mermaid files
    all_files = list(sample_dir.rglob("*.mermaid"))
    total_files = len(all_files)
    
    if total_files == 0:
        print("No .mermaid files found")
        return
    
    print(f"Found {total_files} total .mermaid files")
    
    # Check a random sample of files
    sample_size = min(50, total_files)
    sample_files = random.sample(all_files, sample_size)
    
    valid_count = 0
    invalid_files = []
    
    print(f"\nChecking {sample_size} random samples...")
    
    for file_path in sample_files:
        result = check_sample_structure(file_path)
        if result['valid']:
            valid_count += 1
        else:
            invalid_files.append((file_path, result))
    
    print(f"\nVerification Results:")
    print(f"  Valid samples: {valid_count}/{sample_size}")
    print(f"  Success rate: {(valid_count/sample_size)*100:.1f}%")
    
    if invalid_files:
        print(f"\nInvalid files found:")
        for file_path, result in invalid_files[:5]:  # Show first 5 invalid files
            print(f"  {file_path}: {result}")
    
    # Show summary by type
    type_summary = {}
    for file_path in all_files:
        diagram_type = file_path.parent.name
        type_summary[diagram_type] = type_summary.get(diagram_type, 0) + 1
    
    print(f"\nSummary by diagram type:")
    for diagram_type, count in sorted(type_summary.items()):
        print(f"  {diagram_type}: {count} files")
    
    # Show a few example file contents
    print(f"\nExample file contents:")
    for i, file_path in enumerate(random.sample(all_files, min(3, total_files))):
        print(f"\n--- Example {i+1}: {file_path} ---")
        try:
            with open(file_path, 'r', encoding='utf-8') as f:
                content = f.read()
            print(content[:300] + "..." if len(content) > 300 else content)
        except Exception as e:
            print(f"Error reading file: {e}")

if __name__ == "__main__":
    main()