#!/usr/bin/env python3
"""Train the learned hop scorer (L2 logistic regression) used by src/paths.rs.

Input: hop dumps written by `NEOCORTEX_DUMP_HOPS=<file> neocortex eval <set>
--save-db <db>`, one JSON record per candidate hop the semantic beam scored.
A candidate is positive when one of its evidence memories is an expected
(gold) memory of its query. Gold memories are joined through `<db>.keys.json`
(fixture key -> memory id) and the set's `expect` lists.

Usage:
  python3 scripts/train_hop_scorer.py --source EVAL.json DUMP.jsonl KEYS.json \
      [--source ...] [--val-frac 0.2] [--seed 0] [--out weights.json]

Split is by query (never by candidate row). The L2 strength is chosen on the
validation queries by log-loss; the final weights refit on every query.
Feature order must match `learned_hop_score` in src/paths.rs. Needs numpy only.
"""
import argparse
import json
import random

import numpy as np

FEATURES = [
    "cos_question",
    "cos_residual",
    "hop_index",
    "prose",
    "pred_overlap",
    "log_degree",
    "far_mentioned",
]
PROSE_HOP = 0.8  # src/paths.rs PROSE_HOP: the cosine baseline weights prose hops
LAMBDAS = [0.0, 1e-3, 1e-2, 1e-1, 1.0]


def load_source(index, eval_path, dump_path, keys_path):
    with open(eval_path) as f:
        queries = json.load(f)["queries"]
    with open(keys_path) as f:
        keys = json.load(f)
    gold = {}
    for q in queries:
        expect = frozenset(keys[k] for k in q.get("expect", []))
        gold.setdefault(q["q"], set()).add(expect)
    ambiguous = {text for text, sets in gold.items() if len(sets) > 1}
    rows = []
    skipped = 0
    with open(dump_path) as f:
        for line in f:
            rec = json.loads(line)
            text = rec["query"]
            if text in ambiguous or text not in gold:
                skipped += 1
                continue
            (expect,) = gold[text]
            label = 1.0 if expect.intersection(rec["memory_ids"]) else 0.0
            rows.append((text, rec["features"], label))
    return index, rows, skipped, len(ambiguous)


def sigmoid(z):
    return 1.0 / (1.0 + np.exp(-z))


def fit(X, y, lam):
    """Newton's method on the L2-penalised log-likelihood (bias unpenalised).

    Returns raw-scale weights and bias, so the Rust side applies them directly.
    """
    mu = X.mean(axis=0)
    sd = X.std(axis=0)
    sd[sd == 0] = 1.0
    Z = np.hstack([np.ones((len(X), 1)), (X - mu) / sd])
    penalty = lam * np.eye(Z.shape[1])
    penalty[0, 0] = 0.0
    w = np.zeros(Z.shape[1])
    for _ in range(100):
        p = sigmoid(Z @ w)
        grad = Z.T @ (p - y) + penalty @ w
        hess = Z.T @ (Z * (p * (1 - p))[:, None]) + penalty
        step = np.linalg.solve(hess, grad)
        w -= step
        if np.max(np.abs(step)) < 1e-9:
            break
    ws = w[1:]
    return ws / sd, w[0] - np.sum(ws * mu / sd)


def predict(X, weights, bias):
    return sigmoid(X @ weights + bias)


def logloss(y, p):
    p = np.clip(p, 1e-12, 1 - 1e-12)
    return float(-np.mean(y * np.log(p) + (1 - y) * np.log(1 - p)))


def auc(y, s):
    """Mann-Whitney AUC with average ranks for ties."""
    uniq, inv, counts = np.unique(s, return_inverse=True, return_counts=True)
    avg_rank = np.cumsum(counts) - (counts - 1) / 2.0
    ranks = avg_rank[inv]
    pos = y == 1
    n_pos = int(pos.sum())
    n_neg = len(y) - n_pos
    if n_pos == 0 or n_neg == 0:
        return float("nan")
    return float((ranks[pos].sum() - n_pos * (n_pos + 1) / 2) / (n_pos * n_neg))


def rust_float(v):
    s = f"{v:.9g}"
    return s if ("." in s or "e" in s) else s + ".0"


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--source", nargs=3, action="append", required=True,
                    metavar=("EVAL", "DUMP", "KEYS"))
    ap.add_argument("--val-frac", type=float, default=0.2)
    ap.add_argument("--seed", type=int, default=0)
    ap.add_argument("--out")
    args = ap.parse_args()

    texts, X_rows, y_rows, group_of = [], [], [], []
    group_ids = {}
    for index, (eval_path, dump_path, keys_path) in enumerate(args.source):
        _, rows, skipped, ambiguous = load_source(index, eval_path, dump_path, keys_path)
        print(f"source {index}: {dump_path}: {len(rows)} candidates, "
              f"{skipped} skipped, {ambiguous} ambiguous query texts")
        for text, feats, label in rows:
            key = (index, text)
            gid = group_ids.setdefault(key, len(group_ids))
            X_rows.append(feats)
            y_rows.append(label)
            group_of.append(gid)

    X = np.asarray(X_rows, dtype=float)
    y = np.asarray(y_rows, dtype=float)
    g = np.asarray(group_of)
    n_groups = len(group_ids)
    print(f"total: {len(y)} candidates, {int(y.sum())} positive, {n_groups} queries")

    order = list(range(n_groups))
    random.Random(args.seed).shuffle(order)
    val_groups = set(order[: round(args.val_frac * n_groups)])
    is_val = np.array([gid in val_groups for gid in g])
    train, val = ~is_val, is_val

    best = None
    for lam in LAMBDAS:
        w, b = fit(X[train], y[train], lam)
        loss = logloss(y[val], predict(X[val], w, b))
        print(f"lambda {lam:g}: validation logloss {loss:.5f}")
        if best is None or loss < best[0]:
            best = (loss, lam)
    lam = best[1]

    w_tr, b_tr = fit(X[train], y[train], lam)
    train_auc = auc(y[train], predict(X[train], w_tr, b_tr))
    val_auc = auc(y[val], predict(X[val], w_tr, b_tr))
    cos_base = X[:, 1] * np.where(X[:, 3] == 1.0, PROSE_HOP, 1.0)
    print(f"chosen lambda {lam:g}")
    print(f"train AUC {train_auc:.4f}  validation AUC {val_auc:.4f}")
    print(f"baseline cosine AUC on validation {auc(y[val], cos_base[val]):.4f}")

    w, b = fit(X, y, lam)
    print("final refit on all queries:")
    for name, value in zip(FEATURES, w):
        print(f"  {name:14s} {value: .6f}")
    print(f"  {'bias':14s} {b: .6f}")
    print("const HOP_WEIGHTS: [f64; 7] = ["
          + ", ".join(rust_float(v) for v in w) + "];")
    print(f"const HOP_BIAS: f64 = {rust_float(b)};")

    if args.out:
        with open(args.out, "w") as f:
            json.dump({
                "features": FEATURES,
                "weights": [float(v) for v in w],
                "bias": float(b),
                "lambda": lam,
                "train_auc": train_auc,
                "validation_auc": val_auc,
                "seed": args.seed,
                "val_frac": args.val_frac,
            }, f, indent=2)


if __name__ == "__main__":
    main()
