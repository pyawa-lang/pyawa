# socket 的可移植面：socketpair 收发
import socket

left, right = socket.socketpair()
try:
    left.sendall(b"ping")
    data = right.recv(4)
    print("recv", data)
    right.sendall(b"pong")
    print("recv2", left.recv(4))
finally:
    left.close()
    right.close()
print("closed")
