# 三引号／跨行（第 252 轮）：普通三引号、跨行 f-string、含转义与插值的混排
# 探针一律布尔（值里含换行会打断行式协议）
plain = """a
b"""
plain_ok = plain == "a\nb"
name = "x"
interpolated = f"""a
{name}
c"""
interpolated_ok = interpolated == "a\nx\nc"
escaped = """a\tb"""
escaped_ok = escaped == "a\tb"
