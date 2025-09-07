#!/usr/bin/env python3

import requests
import sys
import os

def test_json_upload():
    """Test JS upload using JSON payload"""
    print("🧪 Testing JSON upload...")
    
    url = "http://127.0.0.1:5074/upload-js"
    
    # Sample JS function
    js_code = """// @name: ADD_NUMBERS
// @description: Adds two numbers together
// @example: ADD_NUMBERS(5, 3) returns 8
// @min_args: 2
// @max_args: 2

function execute(args) {
    return args[0] + args[1];
}
"""
    
    payload = {
        "filename": "test_add.js",
        "js_code": js_code
    }
    
    try:
        response = requests.post(
            url,
            json=payload,
            headers={"Content-Type": "application/json"},
            timeout=10
        )
        
        if response.status_code == 200:
            result = response.json()
            print(f"  ✅ JSON Upload SUCCESS")
            print(f"  📄 Message: {result.get('message', 'N/A')}")
            print(f"  🔧 Function: {result.get('function_name', 'N/A')}")
            if result.get('validation_results'):
                validation = result['validation_results']
                print(f"  ✓ Syntax Valid: {validation.get('syntax_valid', False)}")
                print(f"  ✓ Structure Valid: {validation.get('structure_valid', False)}")
                print(f"  ✓ Example Test: {validation.get('example_test_passed', False)}")
                if validation.get('example_result'):
                    print(f"  📊 Example Result: {validation['example_result']}")
        else:
            print(f"  ❌ JSON Upload FAILED - Status: {response.status_code}")
            print(f"  📄 Response: {response.text}")
            
    except Exception as e:
        print(f"  💥 ERROR - {e}")

def test_multipart_upload():
    """Test JS upload using multipart form-data with file"""
    print("\n🧪 Testing Multipart file upload...")
    
    url = "http://127.0.0.1:5074/upload-js"
    
    # Create a temporary JS file
    js_content = """// @name: MULTIPLY
// @description: Multiplies two numbers
// @example: MULTIPLY(4, 5) returns 20
// @min_args: 2
// @max_args: 2

function execute(args) {
    return args[0] * args[1];
}
"""
    
    try:
        # Test multipart with file upload
        files = {
            'file': ('test_multiply.js', js_content, 'application/javascript')
        }
        data = {
            'filename': 'test_multiply.js'
        }
        
        response = requests.post(
            url,
            files=files,
            data=data,
            timeout=10
        )
        
        if response.status_code == 200:
            result = response.json()
            print(f"  ✅ Multipart Upload SUCCESS")
            print(f"  📄 Message: {result.get('message', 'N/A')}")
            print(f"  🔧 Function: {result.get('function_name', 'N/A')}")
            if result.get('validation_results'):
                validation = result['validation_results']
                print(f"  ✓ Syntax Valid: {validation.get('syntax_valid', False)}")
                print(f"  ✓ Structure Valid: {validation.get('structure_valid', False)}")
                print(f"  ✓ Example Test: {validation.get('example_test_passed', False)}")
                if validation.get('example_result'):
                    print(f"  📊 Example Result: {validation['example_result']}")
        else:
            print(f"  ❌ Multipart Upload FAILED - Status: {response.status_code}")
            print(f"  📄 Response: {response.text}")
            
    except Exception as e:
        print(f"  💥 ERROR - {e}")

def test_update_functionality():
    """Test JS update using JSON payload"""
    print("\n🧪 Testing JSON update...")
    
    url = "http://127.0.0.1:5074/update-js"
    
    # Updated JS function
    js_code = """// @name: ADD_NUMBERS
// @description: Adds two numbers together with updated logic
// @example: ADD_NUMBERS(10, 15) returns 25
// @min_args: 2
// @max_args: 2

function execute(args) {
    // Updated version with validation
    if (typeof args[0] !== 'number' || typeof args[1] !== 'number') {
        throw new Error('Both arguments must be numbers');
    }
    return args[0] + args[1];
}
"""
    
    payload = {
        "filename": "test_add.js",
        "js_code": js_code
    }
    
    try:
        response = requests.put(
            url,
            json=payload,
            headers={"Content-Type": "application/json"},
            timeout=10
        )
        
        if response.status_code == 200:
            result = response.json()
            print(f"  ✅ JSON Update SUCCESS")
            print(f"  📄 Message: {result.get('message', 'N/A')}")
            print(f"  🔧 Function: {result.get('function_name', 'N/A')}")
        else:
            print(f"  ❌ JSON Update FAILED - Status: {response.status_code}")
            print(f"  📄 Response: {response.text}")
            
    except Exception as e:
        print(f"  💥 ERROR - {e}")

def test_list_functions():
    """List uploaded JS functions"""
    print("\n🧪 Listing JS functions...")
    
    url = "http://127.0.0.1:5074/list-js"
    
    try:
        response = requests.get(url, timeout=10)
        
        if response.status_code == 200:
            result = response.json()
            print(f"  ✅ List SUCCESS - Found {result.get('total_count', 0)} functions")
            for func in result.get('functions', []):
                print(f"  📄 {func['filename']} -> {func.get('function_name', 'N/A')}")
        else:
            print(f"  ❌ List FAILED - Status: {response.status_code}")
            print(f"  📄 Response: {response.text}")
            
    except Exception as e:
        print(f"  💥 ERROR - {e}")

if __name__ == "__main__":
    port = "5074"
    if len(sys.argv) > 1:
        port = sys.argv[1]
    
    print(f"🚀 Testing JS Management Endpoints on port {port}")
    print("="*60)
    print("Make sure the HTTP server is running:")
    print(f"Example: ./target/release/sk_http_server {port} --threads 4")
    print()
    
    # Run tests
    test_json_upload()
    test_multipart_upload()
    test_update_functionality()
    test_list_functions()
    
    print("\n🎯 Test Summary:")
    print("- JSON upload: Tests direct JS code in request body")
    print("- Multipart upload: Tests file upload functionality") 
    print("- Update: Tests updating existing JS functions")
    print("- List: Shows all uploaded functions")