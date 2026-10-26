"""Reference logits for tests/fixtures/rerank_fixture.json (fp32, transformers/torch).

Needs torch + transformers (any venv; nothing is installed system-wide):
  python gen_rerank_fixture.py <model_dir> tests/fixtures/rerank_fixture.json
"""
import json
import sys

import torch
from transformers import AutoModelForSequenceClassification, AutoTokenizer

MAX_LEN = 256  # src/rerank.rs MAX_PAIR_TOKENS

model_dir, out = sys.argv[1], sys.argv[2]
tok = AutoTokenizer.from_pretrained(model_dir)
model = AutoModelForSequenceClassification.from_pretrained(model_dir).eval()

long_text = " ".join(
    f"Sentence {i} says the quarterly report for the data platform team was reviewed by Maria Alvarez and Chen Wei."
    for i in range(60)
)
berlin = "How many people live in Berlin?"
pairs = [
    # Model card (sentence-transformers example): [8.607138, -4.320078]
    (berlin, "Berlin had a population of 3,520,031 registered inhabitants in an area of 891.82 square kilometers."),
    (berlin, "Berlin is well known for its museums."),
    # Model card (transformers example), no printed reference there.
    (berlin, "Berlin has a population of 3,520,031 registered inhabitants in an area of 891.82 square kilometers."),
    (berlin, "New York City is famous for the Metropolitan Museum of Art."),
    # Memory-like statements, the actual use.
    ("who is Ravi's partner", "Ravi Menon is married to Meera Iyer."),
    ("who is Ravi's partner", "Ravi Menon works at Initech as a data engineer."),
    ("what does Alex drink in the morning", "Alex switched from coffee to green tea in March 2026."),
    ("what does Alex drink in the morning", "The production database runs PostgreSQL 16."),
    ("where did Noor move", "Noor Haddad relocated to Lisbon in 2025 after leaving Cairo."),
    # Accents and punctuation go through the BERT normaliser.
    ("café in Münster", "Das Café Müller in Münster serves excellent Kaffee."),
    # Longer than MAX_LEN tokens: exercises truncation and padding in one batch.
    ("who reviewed the quarterly report", long_text),
    ("short", "a"),
]

rows = []
for q, t in pairs:
    enc = tok(q, t, truncation=True, max_length=MAX_LEN, return_tensors="pt")
    with torch.no_grad():
        logit = model(**enc).logits[0, 0].item()
    rows.append({"query": q, "text": t, "logit": logit, "tokens": int(enc["input_ids"].shape[1])})
    print(f"{logit:10.6f} {rows[-1]['tokens']:4d}  {q[:40]!r} / {t[:50]!r}")

json.dump({"max_tokens": MAX_LEN, "pairs": rows}, open(out, "w"), indent=1, ensure_ascii=False)
