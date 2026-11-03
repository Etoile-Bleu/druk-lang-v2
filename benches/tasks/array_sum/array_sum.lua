local xs = {}
local i = 0
while i < 100000 do
    table.insert(xs, i)
    i = i + 1
end

local total = 0
i = 0
while i < #xs do
    total = total + xs[i + 1]
    i = i + 1
end
print(total)
