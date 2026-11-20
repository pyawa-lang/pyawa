# time 的可移植面：UTC 固定时刻（本地时区不进输出）
import time

tm = time.gmtime(0)
print("utc", tm.tm_year, tm.tm_mon, tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec)
print("wday", tm.tm_wday, tm.tm_yday)
print("strftime", time.strftime("%Y-%m-%d %H:%M:%S", tm))
print("gmtime_rt", time.gmtime(86400)[:6])
