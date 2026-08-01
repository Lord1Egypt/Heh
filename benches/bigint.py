def factorial(n):
    acc = 1
    for i in range(1, n + 1):
        acc *= i
    return acc

digits = 0
for _round in range(40):
    digits = len(str(factorial(300)))
print(digits)
