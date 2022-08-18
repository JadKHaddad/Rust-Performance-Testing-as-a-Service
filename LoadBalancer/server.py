from waitress import serve
import app

host = "localhost"
port = 6000

if __name__ == '__main__':
    print(f"WAITRESS: Serving on [{host}:{port}]")
    serve(app.app, host=host, port=port)
