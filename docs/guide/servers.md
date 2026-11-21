# Skillet Server Usage Guide

This guide shows how to use the high-performance Skillet server for fast expression evaluation.

## Quick Start

### 1. Start the Server

```bash
# Start on port 8080 with auto-detected CPU cores
./target/release/sk_server 8080

# Or specify number of worker threads (recommended: CPU cores)
./target/release/sk_server 8080 8

or just sk_server 8080 if you instelled the binaries

```

Server output:
```
🚀 Skillet Server started on port 8080
📊 Worker threads: 8
🔧 Ready for high-throughput expression evaluation
```

### 2. Use the Client

```bash
# Basic arithmetic
./target/release/sk_client localhost:8080 "=2 + 3 * 4"
# Output: 14

# With variables
./target/release/sk_client localhost:8080 "=SUM(:sales, :bonus)" sales=1000 bonus=500
# Output: 1500

# With JSON variables
./target/release/sk_client localhost:8080 "=:user.name" --json '{"user": {"name": "Alice"}}'
# Output: "Alice"

# Benchmark performance
./target/release/sk_client localhost:8080 --benchmark "=2+3*4" 1000
```

## Protocol Details

### Request Format (JSON over TCP)

The server accepts JSON requests over TCP, one request per line:

```json
{
  "expression": "=SUM(:a, :b, :c)",
  "variables": {"a": 10, "b": 20, "c": 30},
  "output_json": true
}
```

### Response Format

```json
{
  "success": true,
  "result": 60,
  "error": null,
  "execution_time_ms": 0.123,
  "request_id": 12345
}
```

### Request limits

Both servers (`sk_server` and `sk_http_server`) apply the same request limits. They are
constants; no flag changes them.

| Limit | Value | `sk_server` behavior |
|---|---|---|
| Request size | 1 MB (1,048,576 bytes) per request line | answers `"error": "Request too large (max 1MB)"`, then closes the connection |
| Read deadline | 30 s to receive the whole line | answers `"error": "Request read timed out (max 30 s)"`, then closes |
| Idle connection | 30 s without a real request (blank and invalid lines do not count) | closes the connection without a response |
| Wrong or missing token | one attempt per connection | answers `"error": "Unauthorized: invalid token"`, then closes |
| Write deadline | 10 s to write the whole response | stops writing and closes; the line has no trailing newline |

Pooled connections (see [Connection Pooling](#1-connection-pooling-python)) that sit idle for
30 s are closed by the server, so a pool must reconnect when a send or receive fails.

The write deadline is a total for the whole response, on both servers. A client that reads a
large result more slowly than that gets a truncated response with no error message: on
`sk_server` a JSON line without its trailing `\n`, on `sk_http_server` a body shorter than
`Content-Length`. Sending the largest possible result (256 MiB) within 10 s needs about 25 MB/s.
Details and measurements: [limits](limits.md#response-write-deadline-10-s).

`sk_http_server` answers an oversized request (headers + body over 1 MB, or a `Content-Length`
over 1 MB) and a request that takes more than 30 s to arrive with HTTP status **400** and
`{"error":"413 Payload Too Large","success":false}` or
`{"error":"408 Request Timeout","success":false}`. Evaluation limits (depth, budget, the 10 s
evaluation deadline) are separate; see [limits](limits.md#server-request-limits-1-mb-30-s-10-s)
for both, with captured output.

### Connections and authentication

Start `sk_server` with `--token <secret>` (or `SKILLET_AUTH_TOKEN`) to require a token in every
request's `token` field. An empty token is refused at start-up, with exit code 1:

```console
$ sk_server 5602 --token ""
Error: the auth token is empty (--token or SKILLET_AUTH_TOKEN); give a non-empty token or unset it
$ echo $?
1
$ SKILLET_AUTH_TOKEN= sk_server 5602
Error: the auth token is empty (--token or SKILLET_AUTH_TOKEN); give a non-empty token or unset it
```

A request with a wrong or missing token gets one error response, and then the server closes the
connection. Later lines on that connection are not answered; open a new connection to try again.
Output from a release build of `develop` at 21a37cc, server started with `--token s3cret`:

```console
send: {"expression":"=1+1","token":"wrong"}
      {"expression":"=1+1","token":"s3cret"}
recv: {"success":false,"result":null,"error":"Unauthorized: invalid token","execution_time_ms":0.0,"request_id":0}
      (connection closed; the second line is not answered)

send: {"expression":"=1+1"}
recv: {"success":false,"result":null,"error":"Unauthorized: invalid token","execution_time_ms":0.0,"request_id":1}
      (connection closed)

send: {"expression":"=2+3*4","token":"s3cret"}
recv: {"success":true,"result":14.0,"error":null,"execution_time_ms":0.105916,"request_id":2}
```

A connection stays open as long as it sends a real request at least every 30 s. Blank lines are
ignored and invalid JSON gets an `Invalid JSON request: …` error, but neither restarts the 30 s.
In the same run, a connection that sent one valid request and then a blank or invalid line every
second was closed by the server after 31.1 s, about 30 s after its only real request.

`sk_http_server` refuses empty tokens the same way:

```console
$ sk_http_server 5601 --token ""
Error: --token must not be empty
$ sk_http_server 5601 --admin-token ""
Error: --admin-token must not be empty
```

Both exit with code 1.

`sk_http_server` also reads its tokens from the environment: `SKILLET_AUTH_TOKEN` (eval token)
and `SKILLET_ADMIN_TOKEN` (admin token, for the JavaScript endpoints and `DELETE /cache`). The
flag wins over the variable, and an empty variable is refused like an empty flag:

```console
$ SKILLET_ADMIN_TOKEN= sk_http_server 5611
Error: SKILLET_ADMIN_TOKEN must not be empty; give a non-empty value or unset it
```

If only one token is set, the other one inherits it (the banner warns about it).

### Cross-origin requests and request bodies (`sk_http_server`)

Browsers only let a page read a response from another origin when the response allows it
([ADR-0019](../adr/0019-server-cors-and-development-mode.md)). `sk_http_server` sends
`Access-Control-Allow-Origin` as follows:

| Configuration | `Access-Control-Allow-Origin` |
|---|---|
| `--cors-origin <origin>` or `SKILLET_CORS_ORIGIN` (flag wins) | exactly that value; `*` is allowed |
| no origin, a token configured | `*` |
| no origin, no token (development mode) | not sent |

So a server in development mode cannot be read by a page on another origin. Preflight
`OPTIONS` requests get the same header plus `Access-Control-Allow-Methods: GET, POST, PUT,
DELETE, OPTIONS` and `Access-Control-Allow-Headers: Authorization, Content-Type`; in
development mode they get no CORS headers, and the browser drops the request. The banner shows
the policy, for example `🌍 CORS: Access-Control-Allow-Origin: https://app.example` or
`🌍 CORS: off (no token, no --cors-origin)`.

A request with a body (other than `GET` and `OPTIONS`) must send `Content-Type:
application/json`; parameters such as `; charset=utf-8` are allowed and case does not matter.
Otherwise the answer is HTTP **415**, before any token check. Browsers send a JSON request
from another origin only after a preflight, so a page cannot post to a local server without the
server's consent. `/upload-js` and `/update-js` also accept `multipart/form-data`.

In development mode (no admin token) the admin endpoints (`/upload-js`, `/update-js`,
`/delete-js`, `/list-js`, `/reload-hooks`, `DELETE /cache`) answer HTTP **403** to any request
with an `Origin` header, which browsers send, telling you to set an admin token. curl and
scripts, which send no `Origin`, are not affected.

```console
$ curl -si -X POST localhost:5611/eval -d '{"expression":"=1"}'
HTTP/1.1 415 Unsupported Media Type
Content-Type: application/json
Content-Length: 65
Connection: close

{"error":"Content-Type must be application/json","success":false}
```

(`curl -d` sends `application/x-www-form-urlencoded`; add `-H "Content-Type: application/json"`.)

### JavaScript hook files (`sk_http_server`)

`sk_http_server` loads JavaScript functions from the hooks directory (`SKILLET_HOOKS_DIR`, default
`hooks`), including its subdirectories. The admin endpoints `/upload-js`, `/update-js` and
`/delete-js` accept only a plain file name matching `[A-Za-z0-9_-]+.js`: letters, digits, `_` and
`-`, ending in `.js`, with no directory part. Any other name gets HTTP **400**:

```console
POST /upload-js {"filename": "triple_it.js", "js_code": "…"}
200 {"success":true,"message":"JavaScript function 'TRIPLE_IT' uploaded and validated successfully",…}

POST /upload-js {"filename": "my.func.js", …}
400 {"success":false,"message":"Filename must be a plain name of letters, digits, '_' or '-' ending in .js (no directories)","function_name":null,"validation_results":null,"error":"Invalid filename"}

POST /upload-js {"filename": "triple_it.txt", …}
400 {"success":false,"message":"Filename must end with .js extension","function_name":null,"validation_results":null,"error":"Invalid file extension"}
```

Hooks in subdirectories still load, can be called, and appear in `/list-js` with their relative
path, but they cannot be updated or deleted through the API. Change them on disk and call
`/reload-hooks`:

```console
GET /list-js
200 {"success":true,"functions":[{"filename":"math/double_it.js","function_name":"DOUBLE_IT",…}],"total_count":1,"error":null}

POST /eval {"expression": "=DOUBLE_IT(21)"}
200 {"success":true,"result":42.0,…}

PUT /update-js {"filename": "math/double_it.js", …}
400 {"success":false,"message":"Filename must be a plain name of letters, digits, '_' or '-' ending in .js (no directories)","function_name":null,"validation_results":null,"error":"Invalid filename"}

DELETE /delete-js {"filename": "math/double_it.js"}
400 {"success":false,"message":"Filename must be a plain name of letters, digits, '_' or '-' ending in .js (no directories)","error":"Invalid filename"}
```

(Release build of `develop` at 21a37cc, started with `--admin-token`; the request schema is in
[`openapi.yml`](../../openapi.yml).)

Uploading or updating a hook runs its `@example` once to validate it, under the same limits as any
plugin call (64 MiB, 10 s per call; see [Limits](limits.md#javascript-plugins-64-mib-10-s-256-kib)).
An example that loops answers after 10 s with the timeout in `validation_results.example_error`.
During `/eval` a plugin is also stopped by the 10 s request deadline.

### JavaScript endpoints in builds without plugins

`sk_http_server` built without the default `plugins` feature
(`cargo build --release --no-default-features --bin sk_http_server`) keeps the JavaScript
management routes (`/upload-js`, `/update-js`, `/delete-js`, `/list-js`, `/reload-hooks`) but
does not run them. It checks the admin token as usual (401 when it is wrong) and then answers
HTTP **404**, for any method:

```console
$ curl -s -w ' HTTP %{http_code}\n' localhost:5593/list-js
{"error":"JavaScript plugins are not enabled in this build","success":false} HTTP 404
```

`/eval`, `/health`, `/cache` and the docs routes work in both builds; formulas simply cannot call
JavaScript functions in a plugin-free build.

## Using with Different Languages

### 1. Python Client

```python
import socket
import json

class SkilletClient:
    def __init__(self, host='localhost', port=8080):
        self.host = host
        self.port = port

    def evaluate(self, expression, variables=None):
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            s.connect((self.host, self.port))

            request = {
                'expression': expression,
                'variables': variables,
                'output_json': False
            }

            s.send((json.dumps(request) + '\n').encode())
            response = json.loads(s.recv(4096).decode())

            if response['success']:
                return response['result']
            else:
                raise Exception(response['error'])

# Usage examples
client = SkilletClient()

# Basic calculation
result = client.evaluate('=2 + 3 * 4')
print(f"Result: {result}")  # 14

# With variables
result = client.evaluate('=:price * :quantity', {
    'price': 19.99,
    'quantity': 3
})
print(f"Total: ${result}")  # $59.97

# Array operations
result = client.evaluate('=SUM(:numbers)', {
    'numbers': [1, 2, 3, 4, 5]
})
print(f"Sum: {result}")  # 15
```

### 2. Node.js Client

```javascript
const net = require('net');

class SkilletClient {
    constructor(host = 'localhost', port = 8080) {
        this.host = host;
        this.port = port;
    }

    evaluate(expression, variables = null) {
        return new Promise((resolve, reject) => {
            const client = net.createConnection(this.port, this.host);

            const request = {
                expression,
                variables,
                output_json: false
            };

            client.write(JSON.stringify(request) + '\n');

            client.on('data', (data) => {
                const response = JSON.parse(data.toString());
                client.end();

                if (response.success) {
                    resolve(response.result);
                } else {
                    reject(new Error(response.error));
                }
            });

            client.on('error', reject);
        });
    }
}

// Usage examples
const client = new SkilletClient();

(async () => {
    // Basic calculation
    const result1 = await client.evaluate('=2 + 3 * 4');
    console.log(`Result: ${result1}`);  // 14

    // Financial calculation
    const result2 = await client.evaluate('=:principal * (1 + :rate) ^ :years', {
        principal: 1000,
        rate: 0.05,
        years: 10
    });
    console.log(`Compound interest: $${result2.toFixed(2)}`);

    // String operations
    const result3 = await client.evaluate('=CONCAT(:first, " ", :last)', {
        first: "John",
        last: "Doe"
    });
    console.log(`Full name: ${result3}`);
})();
```

### 3. cURL Commands

```bash
# Using netcat for simple testing
echo '{"expression": "=2+3*4", "variables": null}' | nc localhost 8080

# Test with variables
echo '{"expression": "=SUM(:a, :b)", "variables": {"a": 10, "b": 20}}' | nc localhost 8080

# Complex JSON data
echo '{"expression": "=:user.age * 12", "variables": {"user": {"name": "Alice", "age": 25}}}' | nc localhost 8080

# JSONPath queries with JQ function
echo '{"expression": "SUM(JQ(:arguments, \"$.accounts[*].amount\"))", "variables": {"accounts": [{"amount": 100}, {"amount": 200}, {"amount": 300}]}}' | nc localhost 8080

# JSONPath with filtering
echo '{"expression": "AVG(JQ(:arguments, \"$.scores[?(@.subject == '\''math'\'')].value\"))", "variables": {"scores": [{"subject": "math", "value": 85}, {"subject": "english", "value": 92}, {"subject": "math", "value": 78}]}}' | nc localhost 8080
```

### 4. Bash Function

Add this to your `.bashrc` for convenient testing:

```bash
sk_eval() {
    local expr="$1"
    local vars="$2"
    local host="${SKILLET_HOST:-localhost}"
    local port="${SKILLET_PORT:-8080}"

    if [ -z "$vars" ]; then
        echo "{\"expression\": \"$expr\", \"variables\": null}" | nc "$host" "$port"
    else
        echo "{\"expression\": \"$expr\", \"variables\": $vars}" | nc "$host" "$port"
    fi
}

# Usage:
# sk_eval "=2+3*4"
# sk_eval "=SUM(:a, :b)" '{"a": 10, "b": 20}'
```

## Advanced Usage

### 1. Connection Pooling (Python)

For high-throughput applications, reuse connections:

```python
import socket
import json
import threading

class SkilletPool:
    def __init__(self, host='localhost', port=8080, pool_size=5):
        self.host = host
        self.port = port
        self.pool = []
        self.lock = threading.Lock()

        # Pre-create connections
        for _ in range(pool_size):
            conn = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            conn.connect((host, port))
            self.pool.append(conn)

    def evaluate(self, expression, variables=None):
        with self.lock:
            if not self.pool:
                # Create new connection if pool is empty
                conn = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
                conn.connect((self.host, self.port))
            else:
                conn = self.pool.pop()

        try:
            request = {
                'expression': expression,
                'variables': variables,
                'output_json': False
            }

            conn.send((json.dumps(request) + '\n').encode())
            response = json.loads(conn.recv(4096).decode())

            # Return connection to pool
            with self.lock:
                self.pool.append(conn)

            if response['success']:
                return response['result']
            else:
                raise Exception(response['error'])
        except:
            # Don't return broken connection to pool
            conn.close()
            raise

# High-performance usage
pool = SkilletPool(pool_size=10)

# Multiple concurrent evaluations
import concurrent.futures

expressions = [
    ('=2+3*4', None),
    ('=SUM(:nums)', {'nums': [1,2,3,4,5]}),
    ('=:x * :y', {'x': 10, 'y': 20}),
    # ... more expressions
]

with concurrent.futures.ThreadPoolExecutor(max_workers=10) as executor:
    results = list(executor.map(lambda args: pool.evaluate(*args), expressions))

print("Results:", results)
```

### 2. Load Balancing

For extreme throughput, run multiple server instances:

```bash
# Terminal 1
./target/release/sk_server 8081 4 &

# Terminal 2
./target/release/sk_server 8082 4 &

# Terminal 3
./target/release/sk_server 8083 4 &

# Use nginx or haproxy to load balance between ports 8081, 8082, 8083
```

nginx config example:
```nginx
upstream skillet_servers {
    server 127.0.0.1:8081;
    server 127.0.0.1:8082;
    server 127.0.0.1:8083;
}

server {
    listen 8080;

    location / {
        proxy_pass http://skillet_servers;
        proxy_http_version 1.1;
        proxy_set_header Connection "";
    }
}
```

### 3. Docker Deployment

```dockerfile
# Dockerfile
FROM rust:1.81 as builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin sk_server

FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /app/target/release/sk_server .
EXPOSE 8080
CMD ["./sk_server", "8080", "4"]
```

```bash
# Build and run
docker build -t skillet-server .
docker run -p 8080:8080 skillet-server

# Test from host
echo '{"expression": "=2+3*4", "variables": null}' | nc localhost 8080
```

## Performance Monitoring

### 1. Server Metrics

The server logs performance metrics every 1000 requests:
```
Processed 1000 requests, avg execution time: 1.23ms
```

### 2. Client Benchmarking

```bash
# Basic benchmark
./target/release/sk_client localhost:8080 --benchmark "=2+3*4" 1000

# Complex expression benchmark
./target/release/sk_client localhost:8080 --benchmark "=SUM(1,2,3,4,5) * AVG(10,20,30)" 500

# Variable-heavy benchmark
./target/release/sk_client localhost:8080 --benchmark "=:a + :b + :c" 1000
```

Expected output:
```
📊 BENCHMARK RESULTS
====================
Total requests: 1000
Successful: 1000
Failed: 0
Success rate: 100.00%
Throughput: 847.5 requests/second
Average: 1.18ms
P95: 2.34ms
P99: 4.56ms
Improvement: 212x faster than original
```

### 3. System Monitoring

```bash
# Monitor server resources
htop -p $(pgrep sk_server)

# Monitor network connections
netstat -an | grep :8080

# Monitor memory usage
ps -p $(pgrep sk_server) -o pid,rss,vsz,pcpu,pmem,cmd
```

## Troubleshooting

### Server Won't Start
```bash
# Check if port is in use
lsof -i :8080

# Check server logs
./target/release/sk_server 8080 2>&1 | tee server.log
```

### Connection Refused
```bash
# Test server connectivity
telnet localhost 8080

# Check firewall
sudo ufw status
```

### Poor Performance
```bash
# Check server resource usage
top -p $(pgrep sk_server)

# Increase worker threads
./target/release/sk_server 8080 16

# Check network latency
ping localhost
```

### Expression Errors
```bash
# Test expression syntax with original client first
./target/release/sk "=your_expression_here"

# Check server response
echo '{"expression": "=your_expression", "variables": null}' | nc localhost 8080
```

`Expression nested too deeply (max 128)`, `Evaluation budget exceeded (max 256 MB)` and
`Evaluation timed out (max 10 s)` are limit errors, not bugs. See
[limits](limits.md) for what they mean and how to rewrite the formula.

This server architecture provides a foundation for building high-performance applications that require fast mathematical and logical expression evaluation at scale.