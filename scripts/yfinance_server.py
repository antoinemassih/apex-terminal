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
        elif parsed.path == "/options":
            symbol = params.get("symbol", ["AAPL"])[0]
            date = params.get("date", [None])[0]  # YYYY-MM-DD or None for nearest
            ticker = yf.Ticker(symbol)
            try:
                expirations = list(ticker.options)  # list of date strings
                if date and date in expirations:
                    target_date = date
                elif expirations:
                    target_date = expirations[0]
                else:
                    target_date = None

                result = {"expirations": expirations, "calls": [], "puts": []}
                if target_date:
                    chain = ticker.option_chain(target_date)
                    result["date"] = target_date
                    for _, row in chain.calls.iterrows():
                        result["calls"].append({
                            "strike": round(float(row["strike"]), 2),
                            "lastPrice": round(float(row.get("lastPrice", 0)), 2),
                            "bid": round(float(row.get("bid", 0)), 2),
                            "ask": round(float(row.get("ask", 0)), 2),
                            "volume": int(row.get("volume", 0)) if row.get("volume") and str(row.get("volume")) != "nan" else 0,
                            "openInterest": int(row.get("openInterest", 0)) if row.get("openInterest") and str(row.get("openInterest")) != "nan" else 0,
                            "impliedVolatility": round(float(row.get("impliedVolatility", 0)), 4),
                            "inTheMoney": bool(row.get("inTheMoney", False)),
                            "contractSymbol": str(row.get("contractSymbol", "")),
                        })
                    for _, row in chain.puts.iterrows():
                        result["puts"].append({
                            "strike": round(float(row["strike"]), 2),
                            "lastPrice": round(float(row.get("lastPrice", 0)), 2),
                            "bid": round(float(row.get("bid", 0)), 2),
                            "ask": round(float(row.get("ask", 0)), 2),
                            "volume": int(row.get("volume", 0)) if row.get("volume") and str(row.get("volume")) != "nan" else 0,
                            "openInterest": int(row.get("openInterest", 0)) if row.get("openInterest") and str(row.get("openInterest")) != "nan" else 0,
                            "impliedVolatility": round(float(row.get("impliedVolatility", 0)), 4),
                            "inTheMoney": bool(row.get("inTheMoney", False)),
                            "contractSymbol": str(row.get("contractSymbol", "")),
                        })
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Access-Control-Allow-Origin", "*")
                self.end_headers()
                self.wfile.write(json.dumps(result).encode())
            except Exception as e:
                self.send_response(500)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({"error": str(e)}).encode())
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
