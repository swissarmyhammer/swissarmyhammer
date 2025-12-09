# Embedding CLI Examples

This document provides two essential examples of using the `llama-cli embed` command for text embeddings.

## Example 1: Basic Text Embedding

Generate embeddings for a simple text file:

```bash
# Create a sample input file
echo -e "Hello world\nThis is a test\nAnother example text" > sample.txt

# Generate embeddings with default settings
./target/debug/llama-cli embed \
  --model Qwen/Qwen3-Embedding-0.6B-GGUF \
  --input sample.txt \
  --output embeddings.parquet
```

This creates a Parquet file containing embeddings for each line of text in the input file.

## Example 2: Batch Processing with Normalization

Generate normalized embeddings suitable for similarity search:

```bash
# Create a larger input file
echo -e "Machine learning is fascinating\nArtificial intelligence advances rapidly\nNatural language processing improves\nDeep learning models are powerful\nText embeddings capture semantic meaning" > documents.txt

# Generate normalized embeddings with custom batch size
./target/debug/llama-cli embed \
  --model Qwen/Qwen3-Embedding-0.6B-GGUF \
  --input documents.txt \
  --output normalized_embeddings.parquet \
  --batch-size 16 \
  --normalize
```

The `--normalize` flag ensures embeddings have unit length, making them suitable for cosine similarity calculations. The `--batch-size` parameter controls memory usage and processing speed.