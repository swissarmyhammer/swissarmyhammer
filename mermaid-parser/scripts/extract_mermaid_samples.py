#!/usr/bin/env python3
"""
Extract all Mermaid diagram samples from the mermaid-js repository
and save them as individual .mermaid files organized by type.
"""

import os
import re
import json
from pathlib import Path
from typing import List, Dict, Tuple

def extract_mermaid_from_markdown(content: str, file_path: str) -> List[Tuple[str, str, int]]:
    """Extract Mermaid code blocks from markdown content."""
    samples = []
    pattern = r'```mermaid\n(.*?)\n```'
    matches = re.finditer(pattern, content, re.DOTALL)
    
    for i, match in enumerate(matches):
        mermaid_code = match.group(1).strip()
        line_num = content[:match.start()].count('\n') + 1
        samples.append((mermaid_code, f"{file_path}:L{line_num}", i))
    
    return samples

def extract_mermaid_from_html(content: str, file_path: str) -> List[Tuple[str, str, int]]:
    """Extract Mermaid code from HTML files."""
    samples = []
    
    # Pattern for <div class="mermaid">...</div>
    pattern1 = r'<div[^>]*class="mermaid"[^>]*>(.*?)</div>'
    matches = re.finditer(pattern1, content, re.DOTALL | re.IGNORECASE)
    
    sample_count = 0
    for match in matches:
        mermaid_code = match.group(1).strip()
        # Clean up HTML entities and extra spaces
        mermaid_code = re.sub(r'\s+', ' ', mermaid_code)
        mermaid_code = mermaid_code.replace('&lt;', '<').replace('&gt;', '>').replace('&amp;', '&')
        line_num = content[:match.start()].count('\n') + 1
        samples.append((mermaid_code, f"{file_path}:L{line_num}", sample_count))
        sample_count += 1
    
    # Pattern for script tags with mermaid content
    pattern2 = r'(?:var\s+\w+\s*=\s*|const\s+\w+\s*=\s*)?[\'"`]((?:graph|flowchart|sequenceDiagram|gantt|classDiagram|stateDiagram|pie|gitGraph|journey|C4Context|C4Container|C4Component|C4Dynamic|C4Deployment|erDiagram|architecture|timeline|kanban|radar|treemap|sankey|quadrant|xychart|packet|requirement|block|mindmap).*?)[\'"`]'
    matches = re.finditer(pattern2, content, re.DOTALL)
    
    for match in matches:
        mermaid_code = match.group(1).strip()
        line_num = content[:match.start()].count('\n') + 1
        samples.append((mermaid_code, f"{file_path}:L{line_num}", sample_count))
        sample_count += 1
    
    return samples

def extract_mermaid_from_js(content: str, file_path: str) -> List[Tuple[str, str, int]]:
    """Extract Mermaid diagrams from JavaScript/TypeScript files."""
    samples = []
    
    # Pattern for string literals containing mermaid diagrams
    patterns = [
        r'[\'"`]((?:graph|flowchart|sequenceDiagram|gantt|classDiagram|stateDiagram|pie|gitGraph|journey|C4Context|C4Container|C4Component|C4Dynamic|C4Deployment|erDiagram|architecture|timeline|kanban|radar|treemap|sankey|quadrant|xychart|packet|requirement|block|mindmap).*?)[\'"`]',
        r'mermaid:\s*[\'"`](.*?)[\'"`]',
        r'content:\s*[\'"`]((?:graph|flowchart|sequenceDiagram|gantt|classDiagram|stateDiagram|pie|gitGraph|journey|C4Context|C4Container|C4Component|C4Dynamic|C4Deployment|erDiagram|architecture|timeline|kanban|radar|treemap|sankey|quadrant|xychart|packet|requirement|block|mindmap).*?)[\'"`]'
    ]
    
    sample_count = 0
    for pattern in patterns:
        matches = re.finditer(pattern, content, re.DOTALL)
        for match in matches:
            mermaid_code = match.group(1).strip()
            # Clean up escaped characters
            mermaid_code = mermaid_code.replace('\\n', '\n').replace('\\"', '"').replace("\\'", "'")
            line_num = content[:match.start()].count('\n') + 1
            samples.append((mermaid_code, f"{file_path}:L{line_num}", sample_count))
            sample_count += 1
    
    return samples

def detect_diagram_type(mermaid_code: str) -> str:
    """Detect the type of Mermaid diagram from the code."""
    code_lower = mermaid_code.lower().strip()
    
    type_patterns = {
        'flowchart': [r'^flowchart\s', r'^graph\s'],
        'sequence': [r'^sequencediagram'],
        'gantt': [r'^gantt'],
        'class': [r'^classdiagram'],
        'state': [r'^statediagram'],
        'pie': [r'^pie'],
        'git': [r'^gitgraph'],
        'journey': [r'^journey'],
        'c4': [r'^c4context', r'^c4container', r'^c4component', r'^c4dynamic', r'^c4deployment'],
        'er': [r'^erdiagram'],
        'architecture': [r'^architecture'],
        'timeline': [r'^timeline'],
        'kanban': [r'^kanban'],
        'radar': [r'^radar'],
        'treemap': [r'^treemap'],
        'sankey': [r'^sankey'],
        'quadrant': [r'^quadrant'],
        'xy': [r'^xychart'],
        'packet': [r'^packet'],
        'requirement': [r'^requirement'],
        'block': [r'^block'],
        'mindmap': [r'^mindmap']
    }
    
    for diagram_type, patterns in type_patterns.items():
        for pattern in patterns:
            if re.match(pattern, code_lower):
                return diagram_type
    
    return 'misc'

def save_mermaid_sample(mermaid_code: str, diagram_type: str, source_info: str, sample_index: int):
    """Save a Mermaid sample to a .mermaid file."""
    # Create filename
    source_file = source_info.split(':')[0].split('/')[-1].replace('.', '_')
    filename = f"{source_file}_{sample_index:03d}.mermaid"
    
    # Create full path
    output_dir = Path(f"mermaid-samples/{diagram_type}")
    output_dir.mkdir(parents=True, exist_ok=True)
    output_path = output_dir / filename
    
    # Add metadata as comments
    header = f"""// Source: {source_info}
// Type: {diagram_type}
// Generated by mermaid sample extractor

"""
    
    # Write the file
    with open(output_path, 'w', encoding='utf-8') as f:
        f.write(header + mermaid_code + '\n')
    
    return str(output_path)

def process_file(file_path: Path) -> List[str]:
    """Process a single file and extract Mermaid samples."""
    try:
        with open(file_path, 'r', encoding='utf-8', errors='ignore') as f:
            content = f.read()
    except Exception as e:
        print(f"Error reading {file_path}: {e}")
        return []
    
    samples = []
    file_str = str(file_path)
    
    if file_path.suffix in ['.md', '.markdown']:
        samples = extract_mermaid_from_markdown(content, file_str)
    elif file_path.suffix in ['.html', '.htm']:
        samples = extract_mermaid_from_html(content, file_str)
    elif file_path.suffix in ['.js', '.ts', '.jsx', '.tsx']:
        samples = extract_mermaid_from_js(content, file_str)
    elif file_path.suffix == '.mermaid':
        # Direct mermaid file
        samples = [(content.strip(), file_str, 0)]
    
    saved_files = []
    for mermaid_code, source_info, index in samples:
        if mermaid_code and len(mermaid_code.strip()) > 0:
            diagram_type = detect_diagram_type(mermaid_code)
            saved_file = save_mermaid_sample(mermaid_code, diagram_type, source_info, index)
            saved_files.append(saved_file)
            print(f"Extracted {diagram_type} sample from {source_info} -> {saved_file}")
    
    return saved_files

def main():
    """Main function to extract all Mermaid samples."""
    mermaid_repo_path = Path("mermaid")
    
    if not mermaid_repo_path.exists():
        print("Error: mermaid repository not found. Please clone it first.")
        return
    
    print("Starting Mermaid sample extraction...")
    
    # File extensions to process
    extensions = ['.md', '.markdown', '.html', '.htm', '.js', '.ts', '.jsx', '.tsx', '.mermaid']
    
    all_saved_files = []
    processed_count = 0
    
    # Walk through all files in the repository
    for file_path in mermaid_repo_path.rglob('*'):
        if file_path.is_file() and file_path.suffix in extensions:
            # Skip node_modules and other irrelevant directories
            if any(part in str(file_path) for part in ['node_modules', '.git', 'dist', 'build']):
                continue
            
            saved_files = process_file(file_path)
            all_saved_files.extend(saved_files)
            processed_count += 1
            
            if processed_count % 50 == 0:
                print(f"Processed {processed_count} files, extracted {len(all_saved_files)} samples so far...")
    
    print(f"\nExtraction complete!")
    print(f"Processed {processed_count} files")
    print(f"Extracted {len(all_saved_files)} Mermaid samples")
    
    # Generate summary by type
    type_counts = {}
    for file_path in all_saved_files:
        diagram_type = file_path.split('/')[-2]  # Extract type from path
        type_counts[diagram_type] = type_counts.get(diagram_type, 0) + 1
    
    print("\nSamples by type:")
    for diagram_type, count in sorted(type_counts.items()):
        print(f"  {diagram_type}: {count} samples")

if __name__ == "__main__":
    main()