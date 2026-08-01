m = {"seed": 0}
hits = 0
for i in range(60_000):
    m[f"k{i % 500}"] = i
for i in range(60_000):
    if m.get(f"k{i % 500}") is not None:
        hits += 1
print(hits)
