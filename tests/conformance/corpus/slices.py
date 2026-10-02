# 切片（第 214 轮）：常量界进常量池、非常量界走 BINARY_SLICE／BUILD_SLICE
a = [1, 2, 3, 4, 5]
i = 1
j = 4
x = a[1:3]
y = a[i:j]
z = a[::2]
w = a[::-1]
a[1:3] = [9, 9]
v = a[0] + a[3]
