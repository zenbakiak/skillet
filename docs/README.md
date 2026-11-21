# Skillet Documentation Index

Welcome to the Skillet documentation! Here you'll find everything you need to get started with the high-performance expression engine.

## 📚 Documentation Structure

### 🚀 **Getting Started**
- **[README.md](../README.md)** - Quick overview, installation, and examples
- **[DOCUMENTATION.md](../DOCUMENTATION.md)** - Complete user guide and tutorials

### 📖 **Reference Materials**
- **[API_REFERENCE.md](../API_REFERENCE.md)** - Complete function and method reference
- **[Built-in Functions Reference](../DOCUMENTATION.md#built-in-functions-reference)** - Quick function overview

### 🔧 **Deployment & Operations**
- **[SERVER_USAGE_GUIDE.md](../SERVER_USAGE_GUIDE.md)** - HTTP and TCP server setup
- **[DOCKER_DEPLOYMENT_GUIDE.md](../DOCKER_DEPLOYMENT_GUIDE.md)** - Container deployment
- **[BINARY_DISTRIBUTION_GUIDE.md](../BINARY_DISTRIBUTION_GUIDE.md)** - Binary distribution

### ⚡ **Performance & Architecture**
- **[DEVELOPMENT_GUIDE.md](DEVELOPMENT_GUIDE.md)** - Architecture and contributor guide
- Measured performance baseline: BL-005 (`benches/BASELINE.md`)

---

## ✨ What's New

### 🛡️ **Null Safety Features**
- **Ruby-style Conversion Methods**: `null.to_s()`, `"123".to_i()`, etc.
- **Safe Navigation Operator**: `obj&.property&.method()` 
- **Enhanced Error Prevention**: Handle null values gracefully


### 🎯 **Language Enhancements**
- **Safe Method Calls**: Chain methods without null errors
- **Type Conversions**: Ruby-style `to_*()` methods on all types
- **Improved Error Handling**: Better error messages and recovery

---

## 🏃‍♂️ Quick Start

1. **Install Skillet:**
   ```bash
   cargo add skillet
   ```

2. **Try the new features:**
   ```bash
   # Null-safe operations
   cargo run --bin sk -- "null.to_s().length()"  # 0 (no error!)
   
   # Safe navigation
   cargo run --bin sk -- ":obj := {\"name\": null}; :obj&.name&.length()"  # null
   
   # Type conversions
   cargo run --bin sk -- "\"123\".to_i() + 10"  # 133
   ```

3. **Read the docs:**
   - Start with [DOCUMENTATION.md](../DOCUMENTATION.md) for tutorials
   - Reference [API_REFERENCE.md](../API_REFERENCE.md) for complete function list

---

## 🤝 Need Help?

- **Documentation**: All features are documented with examples

---

**Happy coding with Skillet! 🍳⚡**