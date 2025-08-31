#!/usr/bin/env python3
"""
Skillet Python Client Example

Shows how to use the high-performance Skillet server from Python.
Make sure to start the server first: ./target/release/sk_server 8080
"""

import socket
import json
import time
from typing import Optional, Dict, Any

class SkilletClient:
    """High-performance Python client for Skillet server."""
    
    def __init__(self, host: str = 'localhost', port: int = 8080, timeout: float = 5.0):
        self.host = host
        self.port = port
        self.timeout = timeout
    
    def evaluate(self, expression: str, variables: Optional[Dict[str, Any]] = None, structured_output: bool = False) -> Any:
        """
        Evaluate a Skillet expression.
        
        Args:
            expression: The Skillet expression to evaluate (e.g., "=2+3*4")
            variables: Optional dictionary of variables to pass
            structured_output: If True, returns detailed response with timing info
        
        Returns:
            The result of the expression evaluation
        """
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            s.settimeout(self.timeout)
            s.connect((self.host, self.port))
            
            request = {
                'expression': expression,
                'variables': variables,
                'output_json': structured_output
            }
            
            # Send request
            request_json = json.dumps(request) + '\n'
            s.send(request_json.encode('utf-8'))
            
            # Receive response
            response_data = s.recv(4096).decode('utf-8')
            response = json.loads(response_data)
            
            if response['success']:
                return response['result'] if not structured_output else response
            else:
                raise Exception(f"Skillet error: {response['error']}")

def benchmark_performance(client: SkilletClient, expression: str, iterations: int = 100):
    """Benchmark the performance of an expression."""
    print(f"🚀 Benchmarking: {expression}")
    print(f"   Iterations: {iterations}")
    
    times = []
    successful = 0
    
    for i in range(iterations):
        try:
            start_time = time.time()
            result = client.evaluate(expression)
            end_time = time.time()
            
            times.append((end_time - start_time) * 1000)  # Convert to ms
            successful += 1
            
            if i % (iterations // 10) == 0:
                print(".", end="", flush=True)
                
        except Exception as e:
            print(f"\nError in iteration {i}: {e}")
    
    if times:
        avg_time = sum(times) / len(times)
        min_time = min(times)
        max_time = max(times)
        throughput = successful / (sum(times) / 1000)  # ops per second
        
        print(f"\n")
        print(f"   Results: {successful}/{iterations} successful")
        print(f"   Average: {avg_time:.2f}ms")
        print(f"   Min: {min_time:.2f}ms")
        print(f"   Max: {max_time:.2f}ms") 
        print(f"   Throughput: {throughput:.1f} ops/sec")
    else:
        print("   No successful operations!")

def main():
    print("🧮 Skillet Python Client Demo")
    print("=" * 40)
    
    # Create client
    client = SkilletClient()
    
    print("Testing connection to Skillet server...")
    try:
        result = client.evaluate("=1+1")
        print(f"✅ Server is responding! Test result: {result}")
    except Exception as e:
        print(f"❌ Cannot connect to server: {e}")
        print("Make sure to start the server first:")
        print("   ./target/release/sk_server 8080")
        return
    
    print()
    
    # Demo 1: Basic arithmetic
    print("1. Basic Arithmetic")
    expressions = [
        "=2 + 3 * 4",
        "=10 / 2 + 5",
        "=2 ^ 3 ^ 2",
        "=(1 + 2) * (3 + 4)"
    ]
    
    for expr in expressions:
        result = client.evaluate(expr)
        print(f"   {expr:<20} = {result}")
    
    print()
    
    # Demo 2: Built-in functions
    print("2. Built-in Functions")
    function_expressions = [
        "=SUM(1, 2, 3, 4, 5)",
        "=AVG(10, 20, 30)",
        "=MAX(5, 15, 25, 10)",
        "=MIN(100, 200, 50)"
    ]
    
    for expr in function_expressions:
        result = client.evaluate(expr)
        print(f"   {expr:<25} = {result}")
    
    print()
    
    # Demo 3: Variables
    print("3. Variables")
    variable_tests = [
        ("=:x + :y", {"x": 10, "y": 20}),
        ("=:price * :quantity", {"price": 19.99, "quantity": 3}),
        ("=:name", {"name": "Hello World"}),
        ("=SUM(:numbers)", {"numbers": [1, 2, 3, 4, 5]})
    ]
    
    for expr, vars in variable_tests:
        result = client.evaluate(expr, vars)
        print(f"   {expr:<20} = {result:<10} (vars: {vars})")
    
    print()
    
    # Demo 4: JSON objects
    print("4. JSON Objects")
    json_data = {
        "user": {
            "name": "Alice",
            "age": 30,
            "preferences": {
                "theme": "dark",
                "notifications": True
            }
        },
        "scores": [85, 92, 78, 96, 88]
    }
    
    json_expressions = [
        "=:user.name",
        "=:user.age * 12",  # Age in months
        "=:user.preferences.theme",
        "=SUM(:scores)",
        "=AVG(:scores)"
    ]
    
    for expr in json_expressions:
        result = client.evaluate(expr, json_data)
        print(f"   {expr:<25} = {result}")
    
    print()
    
    # Demo 5: Financial calculations
    print("5. Financial Calculations")
    financial_data = {
        "principal": 1000,
        "rate": 0.05,      # 5% annual rate
        "time": 10,        # 10 years
        "monthly_payment": 200
    }
    
    # Compound interest: A = P(1 + r)^t
    compound_interest = client.evaluate("=:principal * (1 + :rate) ^ :time", financial_data)
    print(f"   Compound Interest:     ${compound_interest:.2f}")
    
    # Total payments over time
    total_payments = client.evaluate("=:monthly_payment * 12 * :time", financial_data)
    print(f"   Total Payments:        ${total_payments:.2f}")
    
    print()
    
    # Demo 6: Structured output
    print("6. Structured Output (with timing)")
    structured_result = client.evaluate("=SUM(1, 2, 3, 4, 5)", structured_output=True)
    print(f"   Expression: SUM(1, 2, 3, 4, 5)")
    print(f"   Result: {structured_result['result']}")
    print(f"   Type: {structured_result['type']}")
    print(f"   Execution Time: {structured_result['execution_time']}")
    
    print()
    
    # Demo 7: Performance benchmark
    print("7. Performance Benchmark")
    benchmark_expressions = [
        "=2 + 3 * 4",
        "=SUM(1, 2, 3, 4, 5, 6, 7, 8, 9, 10)",
        "=:a * :b + :c"
    ]
    
    for expr in benchmark_expressions:
        if ":a" in expr:
            # Set up variables for variable test
            original_evaluate = client.evaluate
            client.evaluate = lambda e, v=None: original_evaluate(e, v or {"a": 10, "b": 20, "c": 5})
            benchmark_performance(client, expr, 50)
            client.evaluate = original_evaluate
        else:
            benchmark_performance(client, expr, 50)
        print()
    
    print("✅ Demo completed!")
    print()
    print("💡 Usage Tips:")
    print("   • All expressions should start with '='")
    print("   • Variables are referenced with ':name'")
    print("   • JSON objects support nested access like ':user.name'")
    print("   • Arrays can be passed as Python lists")
    print("   • Server handles concurrent requests efficiently")

if __name__ == "__main__":
    main()