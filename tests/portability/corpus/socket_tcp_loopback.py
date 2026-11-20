# socket 的可移植面：回环 TCP 建连/收发
import socket

listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
listener.bind(("127.0.0.1", 0))
listener.listen(1)
port = listener.getsockname()[1]
client = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
client.connect(("127.0.0.1", port))
server, _ = listener.accept()
try:
    client.sendall(b"hello")
    print("server_recv", server.recv(5))
    server.sendall(b"world")
    print("client_recv", client.recv(5))
finally:
    client.close()
    server.close()
    listener.close()
print("done")
