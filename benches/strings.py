acc = 0
for i in range(40_000):
    s = f"row-{i}"
    acc += len(s.upper())
print(acc)
