#!/usr/bin/env python3
"""
Local embedding server for searchgrep using C2LLM-0.5B.
Run with: python scripts/embedding_server.py
"""

import argparse
import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer
from typing import List

# Lazy load to show startup message first
embedding_model = None
reranker_model = None


def load_embedding_model():
    global embedding_model
    if embedding_model is not None:
        return

    print("Loading C2LLM-0.5B embedding model...", file=sys.stderr)

    try:
        from sentence_transformers import SentenceTransformer

        embedding_model = SentenceTransformer(
            "codefuse-ai/C2LLM-0.5B",
            trust_remote_code=True,
            tokenizer_kwargs={"padding_side": "left"},
        )
        print("Embedding model loaded successfully!", file=sys.stderr)
    except ImportError:
        print("Error: sentence-transformers not installed.", file=sys.stderr)
        print("Install with: pip install sentence-transformers", file=sys.stderr)
        sys.exit(1)
    except Exception as e:
        print(f"Error loading embedding model: {e}", file=sys.stderr)
        sys.exit(1)


def load_reranker_model():
    global reranker_model
    if reranker_model is not None:
        return

    print("Loading cross-encoder reranker model...", file=sys.stderr)

    try:
        from sentence_transformers import CrossEncoder

        # Use a lightweight but effective reranker
        reranker_model = CrossEncoder(
            "cross-encoder/ms-marco-MiniLM-L-6-v2", max_length=512
        )
        print("Reranker model loaded successfully!", file=sys.stderr)
    except ImportError:
        print("Error: sentence-transformers not installed.", file=sys.stderr)
        print("Install with: pip install sentence-transformers", file=sys.stderr)
        sys.exit(1)
    except Exception as e:
        print(f"Error loading reranker model: {e}", file=sys.stderr)
        sys.exit(1)


# Task instruction for code retrieval
CODE_INSTRUCTION = "Represent this code snippet for retrieval: "
QUERY_INSTRUCTION = "Represent this query for searching relevant code: "

# ColBERT-style token embedding model (lighter weight)
colbert_model = None


def load_colbert_model():
    global colbert_model
    if colbert_model is not None:
        return

    print("Loading ColBERT-style token embedding model...", file=sys.stderr)

    try:
        import torch
        from transformers import AutoModel, AutoTokenizer

        # Use a smaller model that gives good token embeddings
        model_name = "microsoft/codebert-base"
        colbert_model = {
            "tokenizer": AutoTokenizer.from_pretrained(model_name),
            "model": AutoModel.from_pretrained(model_name),
        }
        colbert_model["model"].eval()
        print("ColBERT model loaded successfully!", file=sys.stderr)
    except Exception as e:
        print(f"Error loading ColBERT model: {e}", file=sys.stderr)
        # Fall back to using sentence-transformers
        colbert_model = None


def get_token_embeddings(texts: List[str], max_length: int = 128) -> List[dict]:
    """Get token-level embeddings for ColBERT-style matching."""
    load_colbert_model()

    if colbert_model is None:
        # Fallback: use mean-pooled embedding per token window
        return get_fallback_token_embeddings(texts, max_length)

    import torch

    results = []
    tokenizer = colbert_model["tokenizer"]
    model = colbert_model["model"]

    with torch.no_grad():
        for text in texts:
            # Tokenize
            inputs = tokenizer(
                text,
                return_tensors="pt",
                max_length=max_length,
                truncation=True,
                padding=True,
            )

            # Get hidden states
            outputs = model(**inputs)
            token_embeddings = outputs.last_hidden_state[0]  # [seq_len, hidden_dim]

            # Get tokens (for debugging/inspection)
            tokens = tokenizer.convert_ids_to_tokens(inputs["input_ids"][0])

            # Filter out special tokens
            attention_mask = inputs["attention_mask"][0]
            valid_indices = [
                i
                for i, (mask, tok) in enumerate(zip(attention_mask, tokens))
                if mask == 1 and tok not in ["[CLS]", "[SEP]", "[PAD]", "<s>", "</s>"]
            ]

            filtered_embeddings = token_embeddings[valid_indices].tolist()
            filtered_tokens = [tokens[i] for i in valid_indices]

            results.append(
                {
                    "tokens": filtered_tokens,
                    "embeddings": filtered_embeddings,
                    "dimension": token_embeddings.shape[1],
                }
            )

    return results


def get_fallback_token_embeddings(
    texts: List[str], max_length: int = 128
) -> List[dict]:
    """Fallback: split text into chunks and get embeddings for each."""
    load_embedding_model()

    results = []
    chunk_size = 50  # characters per chunk

    for text in texts:
        chunks = [text[i : i + chunk_size] for i in range(0, len(text), chunk_size)]
        if not chunks:
            chunks = [text]

        # Get embeddings for each chunk
        embeddings = embedding_model.encode(chunks, convert_to_numpy=True)

        results.append(
            {
                "tokens": chunks,
                "embeddings": embeddings.tolist(),
                "dimension": embeddings.shape[1]
                if len(embeddings.shape) > 1
                else len(embeddings),
            }
        )

    return results


def get_embeddings(texts: List[str], is_query: bool = False) -> List[List[float]]:
    """Get embeddings for a list of texts."""
    load_embedding_model()

    instruction = QUERY_INSTRUCTION if is_query else CODE_INSTRUCTION
    prefixed_texts = [instruction + text for text in texts]

    embeddings = embedding_model.encode(prefixed_texts, convert_to_numpy=True)
    return embeddings.tolist()


def rerank(query: str, documents: List[str], top_k: int = None) -> List[dict]:
    """Rerank documents using cross-encoder."""
    load_reranker_model()

    if not documents:
        return []

    # Create query-document pairs
    pairs = [[query, doc] for doc in documents]

    # Get scores from cross-encoder
    scores = reranker_model.predict(pairs)

    # Create results with scores
    results = [
        {"index": i, "score": float(score), "document": doc}
        for i, (doc, score) in enumerate(zip(documents, scores))
    ]

    # Sort by score descending
    results.sort(key=lambda x: x["score"], reverse=True)

    # Return top_k if specified
    if top_k is not None:
        results = results[:top_k]

    return results


class EmbeddingHandler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        # Suppress default logging
        pass

    def send_json_response(self, data: dict, status: int = 200):
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(json.dumps(data).encode())

    def do_POST(self):
        if self.path == "/embeddings":
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length)

            try:
                data = json.loads(body)
                texts = data.get("texts", [])
                is_query = data.get("is_query", False)

                if not texts:
                    self.send_json_response({"error": "No texts provided"}, 400)
                    return

                embeddings = get_embeddings(texts, is_query)
                self.send_json_response(
                    {
                        "embeddings": embeddings,
                        "model": "codefuse-ai/C2LLM-0.5B",
                        "dimension": len(embeddings[0]) if embeddings else 0,
                    }
                )
            except json.JSONDecodeError:
                self.send_json_response({"error": "Invalid JSON"}, 400)
            except Exception as e:
                self.send_json_response({"error": str(e)}, 500)

        elif self.path == "/rerank":
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length)

            try:
                data = json.loads(body)
                query = data.get("query", "")
                documents = data.get("documents", [])
                top_k = data.get("top_k")

                if not query:
                    self.send_json_response({"error": "No query provided"}, 400)
                    return

                if not documents:
                    self.send_json_response({"error": "No documents provided"}, 400)
                    return

                results = rerank(query, documents, top_k)
                self.send_json_response(
                    {
                        "results": results,
                        "model": "cross-encoder/ms-marco-MiniLM-L-6-v2",
                    }
                )
            except json.JSONDecodeError:
                self.send_json_response({"error": "Invalid JSON"}, 400)
            except Exception as e:
                self.send_json_response({"error": str(e)}, 500)

        elif self.path == "/colbert_embeddings":
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length)

            try:
                data = json.loads(body)
                texts = data.get("texts", [])
                max_length = data.get("max_length", 128)

                if not texts:
                    self.send_json_response({"error": "No texts provided"}, 400)
                    return

                results = get_token_embeddings(texts, max_length)
                self.send_json_response(
                    {
                        "results": results,
                        "model": "microsoft/codebert-base",
                    }
                )
            except json.JSONDecodeError:
                self.send_json_response({"error": "Invalid JSON"}, 400)
            except Exception as e:
                self.send_json_response({"error": str(e)}, 500)

        elif self.path == "/health":
            self.send_json_response(
                {
                    "status": "ok",
                    "embedding_model": "codefuse-ai/C2LLM-0.5B",
                    "reranker_model": "cross-encoder/ms-marco-MiniLM-L-6-v2",
                    "colbert_model": "microsoft/codebert-base",
                }
            )

        else:
            self.send_json_response({"error": "Not found"}, 404)

    def do_GET(self):
        if self.path == "/health":
            self.send_json_response(
                {
                    "status": "ok",
                    "embedding_model": "codefuse-ai/C2LLM-0.5B",
                    "embedding_ready": embedding_model is not None,
                    "reranker_model": "cross-encoder/ms-marco-MiniLM-L-6-v2",
                    "reranker_ready": reranker_model is not None,
                    "colbert_model": "microsoft/codebert-base",
                    "colbert_ready": colbert_model is not None,
                }
            )
        else:
            self.send_json_response({"error": "Not found"}, 404)


def main():
    parser = argparse.ArgumentParser(
        description="Local embedding server for searchgrep"
    )
    parser.add_argument("--port", type=int, default=11434, help="Port to run server on")
    parser.add_argument("--host", type=str, default="127.0.0.1", help="Host to bind to")
    parser.add_argument("--preload", action="store_true", help="Load models on startup")
    args = parser.parse_args()

    if args.preload:
        load_embedding_model()
        load_reranker_model()

    server = HTTPServer((args.host, args.port), EmbeddingHandler)
    print(
        f"Embedding server running at http://{args.host}:{args.port}", file=sys.stderr
    )
    print("Endpoints:", file=sys.stderr)
    print("  POST /embeddings         - Get embeddings for texts", file=sys.stderr)
    print("  POST /rerank             - Rerank documents for a query", file=sys.stderr)
    print("  POST /colbert_embeddings - Get token-level embeddings", file=sys.stderr)
    print("  GET  /health             - Health check", file=sys.stderr)

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nShutting down...", file=sys.stderr)
        server.shutdown()


if __name__ == "__main__":
    main()
