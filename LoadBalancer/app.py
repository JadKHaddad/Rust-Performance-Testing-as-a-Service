from flask import Flask, Response, request
from load_balancer import LoadBalancer
import requests
from requests.exceptions import ConnectTimeout, InvalidSchema, ReadTimeout, InvalidURL, ConnectionError
from waitress import serve

app = Flask(__name__)
balancer = LoadBalancer()


@app.route('/', defaults={'path': ''})
@app.route('/<path:path>', methods=["POST"])
def index(path):
    server = balancer.pick()
    print(f"LOADBALANCER: Forwarding to [{server}/{path}]")

    try:
        res = requests.post(f"{server}/{path}",
                            data=request.data, headers=request.headers)
        return Response(
            response=res.content, status=res.status_code, headers=dict(
                res.headers)
        )
    except (ConnectTimeout, ConnectionError, InvalidSchema, ReadTimeout, InvalidURL):
        return Response(status=503)


balancer.add("http://localhost:5000")
# balancer.add("http://localhost:5001")

host = "localhost"
port = 6000

if __name__ == '__main__':
    print(
        f"WAITRESS: Serving on [{host}:{port}] | Endpoints: [{balancer.servers}]")
    serve(app, host=host, port=port)
