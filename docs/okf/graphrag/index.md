---
okf_version: "0.2"
---
# GraphRAG for neocortex

What neocortex (an LLM-free, CPU, SQLite agent memory) can take from the papers, benchmarks and
projects listed in [Awesome-GraphRAG](https://github.com/DEEP-PolyU/Awesome-GraphRAG).
Every concept is machine-generated (`status: draft`, unverified) and rates its
`neocortex_relevance` as adopt | adapt | watch | skip.

# Adopt first

* [Answering Complex Open-Domain Questions with Multi-Hop Dense Retrieval](papers/retrieval/multi-hop-dense-retrieval.md) - Recursive dense retrieval that finds the sequence of passages for multi-hop questions, with the next query formed from the question plus previously retrieved passages, using beam search. (neocortex: adopt · LLM: none)
* [Don't Forget the Base Retriever! A Low-Resource Graph-based Retriever for Multi-hop Question Answering](papers/retrieval/dont-forget-the-base-retriever.md) - GRiever: an LLM-free, multi-step graph retriever over a passage-triple index plus synonym expansion, running in hundreds of milliseconds. (neocortex: adopt · LLM: indexing)
* [E^2GraphRAG: Streamlining Graph-based RAG for High Efficiency and Effectiveness](papers/organization/e2graphrag.md) - GraphRAG variant that builds the entity graph with SpaCy (no LLM) plus a summary tree, bidirectional entity-chunk indexes, and adaptive local/global retrieval with no LLM calls at query time. (neocortex: adopt · LLM: indexing)
* [GraphRAG-Bench](benchmarks/graphrag-bench.md) - Benchmark of about 4,000 questions over two corpora (Gutenberg novels and medical guidelines) asking when graph structure helps RAG across four task levels. (neocortex: adopt · LLM: both)
* [GraphRAG-Bench (repository)](projects/graphrag-bench-site.md) - Code repository and dataset host for the GraphRAG-Bench benchmark; the benchmark's method and data are in its benchmark entry. (neocortex: adopt · LLM: both)
* [LinearRAG: Linear Graph Retrieval Augmented Generation on Large-scale Corpora](papers/organization/linearrag.md) - Relation-free Tri-graph of entities, sentences and passages, built with spaCy NER, plus two-stage entity activation and PPR passage ranking; zero LLM tokens for indexing and retrieval. (neocortex: adopt · LLM: none)
* [MetaQA](benchmarks/metaqa.md) - Multi-hop KB question answering benchmark (1-, 2- and 3-hop) over the WikiMovies knowledge base, with Vanilla, NTM-paraphrased and audio variants; about 400K questions in total. (neocortex: adopt · LLM: none)
* [MetaQA-3 (3-hop split)](benchmarks/metaqa-3hop.md) - The 3-hop split of MetaQA: 142,744 template questions over the WikiMovies KB, requiring three-hop logical reasoning (AAAI 2018). (neocortex: adopt · LLM: none)
* [MultiHop-RAG](benchmarks/multihop-rag.md) - RAG benchmark of 2,556 multi-hop news queries with gold evidence sentences, ground-truth answers and null (unanswerable) queries. (neocortex: adopt · LLM: both)

# Directories

* [Cross-cutting techniques](techniques/) - 15 concepts
* [Research papers](papers/) - 109 concepts
* [Benchmarks](benchmarks/) - 19 concepts
* [Open-source projects](projects/) - 16 concepts
* [Related surveys](surveys/) - 3 concepts
