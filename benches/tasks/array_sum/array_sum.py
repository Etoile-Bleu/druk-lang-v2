xs = []
i = 0
while i < 100000:
    xs.append(i)
    i = i + 1

total = 0
i = 0
while i < len(xs):
    total = total + xs[i]
    i = i + 1
print(total)
