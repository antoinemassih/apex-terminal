"""Tiny HTTP server that wraps yfinance. Tauri calls this via reqwest."""
import json, sys
from http.server import HTTPServer, BaseHTTPRequestHandler
from urllib.parse import urlparse, parse_qs
import yfinance as yf

class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        parsed = urlparse(self.path)
        params = parse_qs(parsed.query)
        if parsed.path == "/bars":
            symbol = params.get("symbol", ["AAPL"])[0]
            interval = params.get("interval", ["5m"])[0]
            period = params.get("period", ["5d"])[0]
            ticker = yf.Ticker(symbol)
            df = ticker.history(period=period, interval=interval)
            bars = []
            for ts, row in df.iterrows():
                bars.append({
                    "time": int(ts.timestamp()),
                    "open": round(row["Open"], 4),
                    "high": round(row["High"], 4),
                    "low": round(row["Low"], 4),
                    "close": round(row["Close"], 4),
                    "volume": round(row["Volume"], 2),
                })
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Access-Control-Allow-Origin", "*")
            self.end_headers()
            self.wfile.write(json.dumps(bars).encode())
        elif parsed.path == "/health":
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b"ok")
        else:
            self.send_response(404)
            self.end_headers()
    def log_message(self, format, *args):
        pass

if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8777
    server = HTTPServer(("127.0.0.1", port), Handler)
    print(f"yfinance server on http://127.0.0.1:{port}")
    server.serve_forever()
