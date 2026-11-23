#!/usr/bin/env python3

import requests
import json
import sys

def test_large_request():
    """Test server with progressively larger requests to identify limits"""
    
    base_url = "http://127.0.0.1:5074/eval"
    
    # Test different payload sizes
    test_sizes = [
        ("1KB", 1024),
        ("10KB", 10240), 
        ("30KB", 30720),  # Close to your 34.2KB issue
        ("50KB", 51200),
        ("100KB", 102400),
        ("500KB", 512000),
        ("1MB", 1024000),   # Should hit our limit
        ("2MB", 2048000),   # Should be rejected
    ]
    
    print("🧪 Testing HTTP server with large payloads")
    print("=" * 50)
    
    for size_name, size_bytes in test_sizes:
        print(f"\n📦 Testing {size_name} payload ({size_bytes:,} bytes)")
        
        # Create a large expression by repeating assignments
        assignments_per_line = 50
        lines_needed = max(1, size_bytes // (assignments_per_line * 10))  # Rough estimate
        
        # Generate large expression
        expression_parts = []
        for i in range(lines_needed):
            line = "; ".join([f":var{j+i*assignments_per_line}:={j+i*assignments_per_line}" 
                             for j in range(assignments_per_line)])
            expression_parts.append(line)
        
        large_expression = "; ".join(expression_parts) + "; :result := :var0 + :var1"
        
        payload = {
            "expression": large_expression,
            "include_variables": True
        }
        
        payload_json = json.dumps(payload)
        actual_size = len(payload_json.encode('utf-8'))
        
        print(f"  📏 Actual payload size: {actual_size:,} bytes")
        
        try:
            response = requests.post(
                base_url, 
                json=payload,
                timeout=30,
                headers={"Content-Type": "application/json"}
            )
            
            if response.status_code == 200:
                result = response.json()
                print(f"  ✅ SUCCESS - Status: {response.status_code}")
                print(f"  ⏱️  Execution time: {result.get('execution_time_ms', 'N/A')}ms")
                vars_count = len(result.get('variables', {}))
                print(f"  📊 Variables returned: {vars_count}")
            else:
                print(f"  ❌ HTTP ERROR - Status: {response.status_code}")
                print(f"  📄 Response: {response.text[:200]}...")
                
        except requests.exceptions.ConnectionError as e:
            print(f"  🔌 CONNECTION ERROR - {e}")
            print(f"     This indicates 'Connection reset by peer'")
        except requests.exceptions.Timeout:
            print(f"  ⏱️  TIMEOUT - Request took longer than 30s")
        except requests.exceptions.RequestException as e:
            print(f"  ❌ REQUEST ERROR - {e}")
        except Exception as e:
            print(f"  💥 UNEXPECTED ERROR - {e}")
            
        # Add a small delay between tests
        import time
        time.sleep(0.5)

if __name__ == "__main__":
    if len(sys.argv) > 1:
        port = sys.argv[1]
        base_url = f"http://127.0.0.1:{port}/eval"
    
    print("Make sure the HTTP server is running on port 5074")
    print("Example: ./target/release/sk_http_server 5074 --threads 4")
    print()
    
    test_large_request()
    
    print("\n🎯 Summary:")
    print("- Look for CONNECTION ERROR to identify the breaking point")
    print("- Payloads >1MB should be rejected with HTTP 413")
    print("- Connection resets indicate buffer/timeout issues")