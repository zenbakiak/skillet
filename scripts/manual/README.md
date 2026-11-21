# Manual smoke scripts

Python scripts that poke a running `sk_http_server` on `127.0.0.1:5074`. They are not part of the
test suite or the gate; automated coverage lives in `tests/`. They need `pip install requests`.

| Script | What it does |
|---|---|
| `js_upload_smoke.py` | Uploads a JS function through `/upload-js` (JSON and multipart), then exercises `/update-js` and `/list-js`. Admin endpoints: start the server without an admin token, or adapt the headers |
| `large_request_smoke.py` | Sends `/eval` requests from 1 KB to 2 MB to find the request-size limit (1 MB, see [limits](../../docs/guide/limits.md)) |

```bash
cargo run --release --bin sk_http_server -- 5074 &
python3 scripts/manual/js_upload_smoke.py
python3 scripts/manual/large_request_smoke.py
```
