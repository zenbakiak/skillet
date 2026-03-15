# Skillet Formula Builder

Expert assistant for creating Skillet expressions with Excel-like formulas and Ruby-style methods. Generates optimized formulas for both CLI and HTTP API usage with complete validation and testing.

## Triggers

Use this skill when users ask to:
- "Create a Skillet formula"
- "Write a Skillet expression"
- "Generate Skillet calculation"
- "Build Skillet query"
- "Help me with Skillet syntax"
- "How do I calculate X in Skillet"
- "Convert Excel formula to Skillet"
- "Filter/map/reduce data in Skillet"
- "Query JSON with Skillet"
- "Create business logic with Skillet"

## Core Knowledge

### Skillet Language Essentials

**Variable Syntax:**
- Variables use `:` prefix: `:price`, `:quantity`, `:user`
- Assignment: `:total := :price * :qty`
- Special variable: `:arguments` (auto-populated from JSON input)

**Value Types:**
- Numbers: `42`, `3.14` (all f64)
- Strings: `"hello"`, `'world'`
- Booleans: `true`, `false`, `TRUE`, `FALSE`
- Arrays: `[1, 2, 3]`, `["a", "b", "c"]`
- Null: `null`, `NULL`
- Objects: `{name: "John", age: 30}`

**Operators (Precedence Order):**
```
1. Method calls:           .method()
2. Unary:                  !, -, +
3. Exponentiation:         ^
4. Multiplication/Division: *, /, %
5. Addition/Subtraction:   +, -
6. Comparisons:            >, <, >=, <=, ==, !=
7. Logical AND:            AND, &&
8. Logical OR:             OR, ||
9. Ternary:                ? :
10. Assignment:            :=
```

**Special Operators:**
- Safe navigation: `&.` (prevents null errors)
- Type casting: `::` (e.g., `"42" :: Integer`)
- Spread: `...` (e.g., `SUM(...[1,2,3])`)

**Statement Termination:**
- Semicolon `;` marks the end of a statement
- Multiple statements can be chained: `:x := 10; :y := 20; :x + :y`
- Last expression is returned as the result

**Comments:**
Skillet supports three types of comments for documenting your formulas:

1. **Line Comments (Hash):** `# comment text`
   - Everything after `#` to end of line is ignored
   - Common in Python, Ruby, Shell scripts

2. **Line Comments (Slash):** `// comment text`
   - Everything after `//` to end of line is ignored
   - Common in JavaScript, C, Java, Rust

3. **Block Comments:** `/* comment text */`
   - Everything between `/*` and `*/` is ignored
   - Can span multiple lines or be inline
   - Common in C, JavaScript, CSS

**Comment Examples:**
```skillet
# Calculate total with discount
:qty := 100;
:price := 50; // Unit price in USD
:subtotal := :qty * :price;

/* Apply tiered discount:
   - 20% for qty >= 100
   - 10% for qty >= 50
   - 5% for qty >= 10
*/
:discount := IFS(
  :qty >= 100, /* bulk */ 0.20,
  :qty >= 50, /* medium */ 0.10,
  :qty >= 10, /* small */ 0.05,
  true, 0
);

:total := :subtotal * (1 - :discount); // Final price
:total
```

### HTTP API Expression Formatting

**IMPORTANT:** In HTTP API calls, expressions can be formatted in TWO ways for better readability:

#### Format 1: Single String (Compact)
```json
{
  "expression": ":x := 10; :y := 20; :x + :y"
}
```

#### Format 2: Array of Strings (Readable - RECOMMENDED!)
The expression can be split into an array of strings, where each string is a logical line or statement. The server automatically joins them together.

**Basic Array Format:**
```json
{
  "expression": [
    ":filtered := :products.filter(:x.category == :target_category);",
    ":subtotal := SUM(:filtered.map(:x.price * :x.quantity));",
    ":tax := :subtotal * 0.16;",
    ":total := :subtotal + :tax;",
    ":total"
  ]
}
```

**Advanced Array Format (Indented for Complex Expressions):**
For complex expressions like object literals or nested function calls, you can break them across multiple lines with proper indentation (like JavaScript):

```json
{
  "expression": [
    ":filtered := :products.filter(:x.category == :target_category);",
    ":items_detail := :filtered.map({",
    "  name: :item.name,",
    "  unit_price: :item.price,",
    "  quantity: :item.quantity,",
    "  discount_pct: IFS(",
    "    :item.quantity >= 100, 15,",
    "    :item.quantity >= 50, 10,",
    "    :item.quantity >= 10, 5,",
    "    true, 0",
    "  ),",
    "  subtotal: :item.price * :item.quantity",
    "}, 'item');",
    ":total := SUM(:items_detail.map(:x.subtotal));",
    ":total"
  ]
}
```

**Key Rules for Array Format:**
1. ✅ Each statement ends with semicolon `;` (except the last expression)
2. ✅ Indent like JavaScript for readability
3. ✅ Break complex expressions across multiple strings
4. ✅ Each array element is a string (use proper JSON escaping)
5. ✅ Server joins all strings before evaluation
6. ✅ Whitespace and indentation are preserved for readability

**Benefits of Array Format:**
- 📖 **More readable** - Easy to see the logic flow
- 🔧 **Easier to edit** - Modify individual lines without escaping issues
- 🐛 **Better debugging** - Can comment out lines during testing
- 👥 **Team-friendly** - Code reviews are much clearer
- 📝 **Self-documenting** - Structure shows intent

**When to Use Each Format:**
- **Single String:** Simple formulas (1-2 statements)
- **Array Format:** Complex logic (3+ statements, nested structures, object literals)

### Complete Function Reference

#### Arithmetic Functions
- `SUM(...)` - Sum all arguments
- `PRODUCT(...)`, `MULTIPLY(...)` - Product of all arguments
- `AVERAGE(...)`, `AVG(...)` - Average of arguments
- `MAX(...)`, `MIN(...)` - Maximum/minimum value
- `COUNT(...)` - Count numeric arguments
- `ABS(number)` - Absolute value
- `ROUND(number, [digits])` - Round to decimal places
- `CEILING(number)`, `CEIL(number)` - Round up
- `FLOOR(number)` - Round down
- `INT(number)` - Integer part
- `MOD(dividend, divisor)` - Modulo
- `POWER(base, exp)`, `POW(base, exp)` - Exponentiation
- `SQRT(number)` - Square root

#### Statistical Functions
- `MEDIAN(...)` - Median value
- `MODE.SNGL(...)`, `MODE_SNGL(...)` - Most frequent value
- `STDEV.P(...)`, `STDEV_P(...)` - Population standard deviation
- `VAR.P(...)`, `VAR_P(...)` - Population variance
- `PERCENTILE.INC(array, k)` - k-th percentile (0-1)
- `QUARTILE.INC(array, quart)` - Quartile (1=Q1, 2=Q2, 3=Q3)

#### Logical Functions
- `IF(condition, true_val, false_val)` - Conditional
- `IFS(cond1, val1, cond2, val2, ...)` - Multiple conditions
- `AND(...)` - Logical AND
- `OR(...)` - Logical OR
- `NOT(value)` - Logical NOT
- `XOR(...)` - Exclusive OR

#### Text Functions
- `LEN(text)`, `LENGTH(text)` - String length
- `CONCAT(...)` - Concatenate strings
- `SUBSTRING(text, start, length)` - Extract substring (0-based)
- `LEFT(text, [n])` - Leftmost n characters (default 1)
- `RIGHT(text, [n])` - Rightmost n characters (default 1)
- `MID(text, start, [n])` - Substring (1-based, Excel-style)
- `SUBSTITUTE(text, old, new)` - Replace all occurrences
- `SUBSTITUTEM(text, old, new)` - Same as SUBSTITUTE
- `REPLACE(text, start, num_chars, new)` - Positional replace (1-based)
- `UPPER(text)`, `LOWER(text)` - Change case
- `TRIM(text)` - Remove whitespace
- `REVERSE(text)` - Reverse string
- `SPLIT(text, delimiter)` - Split into array
- `ISNUMBER(value)` - Check if numeric
- `ISTEXT(value)` - Check if text
- `ISBLANK(value)` - Check if blank/null

#### Array Functions
- `ARRAY(...)` - Create array
- `FLATTEN(array)` - Flatten nested arrays
- `UNIQUE(array)` - Remove duplicates
- `COMPACT(array)` - Remove nulls
- `SORT(array, [order])` - Sort array
- `REVERSE(array)` - Reverse array
- `JOIN(array, [delimiter])` - Join to string
- `FIRST(array)`, `LAST(array)` - First/last element
- `CONTAINS(array, value)`, `IN(value, array)` - Check membership

#### Higher-Order Functions (Functional Programming)
- `FILTER(array, :x > condition)` - Filter elements
- `MAP(array, :x * transform)` - Transform elements
- `REDUCE(array, :acc + :x, initial)` - Reduce to single value
- `FIND(array, :x == condition)` - Find first match
- `SUMIF(array, :x > condition)` - Conditional sum (lambda)
- `SUMIF(array, ">5")` - Conditional sum (Excel-style criteria)
- `AVGIF(array, :x > condition)` - Conditional average
- `COUNTIF(array, :x > condition)` - Conditional count

**Excel-Style SUMIF Criteria:**
- `">value"` - Greater than
- `"<value"` - Less than
- `">=value"` - Greater or equal
- `"<=value"` - Less or equal
- `"=value"` - Equal to
- `"<>value"` - Not equal to

#### Date/Time Functions
- `NOW()` - Current Unix timestamp
- `DATE(year, month, day)` - Create date
- `TIME(hour, minute, second)` - Seconds since midnight
- `YEAR(date)`, `MONTH(date)`, `DAY(date)` - Extract components
- `DATEADD(date, amount, unit)` - Add time (units: "days", "months", "years", "weeks", "hours", "minutes")
- `DATEDIFF(date1, date2, [unit])` - Difference between dates

#### Financial Functions
- `PMT(rate, nper, pv, [fv], [type])` - Loan payment calculator
  - `rate`: Interest rate per period
  - `nper`: Number of periods
  - `pv`: Present value (loan amount)
  - `fv`: Future value (optional, default 0)
  - `type`: Payment timing (0=end, 1=beginning)

#### JSON Functions
- `DIG(json, ['path', 'to', index, 'key'], [default])` - Navigate nested JSON
- `JQ(json, "$.jsonpath.expression")` - JSONPath queries
  - `"$.property"` - Access property
  - `"$.array[*]"` - All array elements
  - `"$.array[?(@.field == 'value')]"` - Filter arrays
  - Use with aggregation: `SUM(JQ(:arguments, "$.sales[*].amount"))`

#### Utility Functions
- `BETWEEN(min, max, value)` - Range check (inclusive)

### Ruby-Style Methods

#### String Methods
- `.upper()`, `.lower()` - Change case
- `.trim()` - Remove whitespace
- `.reverse()` - Reverse string
- `.length()` - String length
- `.includes(substring)` - Check if contains
- `.blank?` - Check if empty/null
- `.present?` - Check if not empty

#### Array Methods
- `.length()`, `.size()`, `.count()` - Array length
- `.first()`, `.last()` - First/last element
- `.reverse()` - Reverse array
- `.unique()` - Remove duplicates
- `.compact()` - Remove nulls
- `.sort([order])` - Sort ("ASC" or "DESC")
- `.sum()`, `.avg()`, `.min()`, `.max()` - Aggregations
- `.join([separator])` - Join to string (default: ",")
- `.contains(value)`, `.includes(value)` - Check membership
- `.flatten()` - Flatten nested arrays

#### Array Higher-Order Methods
- `.filter(:x > condition)` - Filter elements
- `.map(:x * transform)` - Transform elements
- `.reduce(:acc + :x, initial)` - Reduce to value
- `.find(:x == condition)` - Find first match

#### Number Methods
- `.abs()` - Absolute value
- `.ceil()`, `.floor()` - Round up/down
- `.round([digits])` - Round to decimals
- `.sqrt()` - Square root
- `.int()` - Integer part
- `.sin()`, `.cos()`, `.tan()` - Trigonometric

#### Number Predicate Methods
- `.positive?` - Is positive
- `.negative?` - Is negative
- `.zero?` - Is zero
- `.even?` - Is even
- `.odd?` - Is odd
- `.between(min, max)` - Range check

#### Type Conversion Methods (Available on ALL types including null!)
- `.to_s()`, `.to_string()` - Convert to string
- `.to_i()`, `.to_int()` - Convert to integer
- `.to_f()`, `.to_float()` - Convert to float
- `.to_a()`, `.to_array()` - Convert to array
- `.to_json()` - Convert to JSON
- `.to_bool()`, `.to_boolean()` - Convert to boolean

**Null Conversions (Safe Defaults):**
- `null.to_s()` → `""` (empty string)
- `null.to_i()` → `0`
- `null.to_f()` → `0.0`
- `null.to_a()` → `[]`
- `null.to_json()` → `"{}"`
- `null.to_bool()` → `false`

#### JSON Object Methods
- `.keys()` - Get object keys
- `.values()` - Get object values
- `.has_key(key)`, `.has(key)` - Check if key exists
- `.dig(['path', 'to', 'key'], [default])` - Safe navigation

### Null Safety Features

**Safe Navigation Operator (`&.`):**
```bash
:user&.profile&.avatar&.url  # Returns null if any part is null (no error!)
```

**Conversion Methods for Null Safety:**
```bash
:value.to_s().length()       # Works even if :value is null (returns 0)
:data.filter(:x.name.to_s().length() > 0)  # Filters out nulls safely
```

**When to Use:**
- Use `&.` when you want to preserve null in the result
- Use `.to_*()` when you want to convert null to a safe default

### Performance Best Practices

1. **Method Chaining:** Chain operations for readability
   ```bash
   [1,2,3,4,5].filter(:x > 2).map(:x * 10).sum()
   ```

2. **Avoid Redundant Operations:** Don't recalculate in loops
   ```bash
   # ✅ Good
   :tax_rate := 1.16; :items.map(:x.price * :tax_rate)

   # ❌ Bad
   :items.map(:x.price * 1.16)  # Harder to maintain
   ```

3. **Use Built-in Functions:** They're optimized
   ```bash
   # ✅ Good
   SUM([1,2,3,4,5])

   # ❌ Bad
   [1,2,3,4,5].reduce(:acc + :x, 0)
   ```

4. **Null Safety:** Use conversion methods or safe navigation
   ```bash
   # ✅ Good
   :data.filter(:x.status.to_s().length() > 0)

   # ❌ Bad (will error if status is null)
   :data.filter(:x.status.length() > 0)
   ```

## Instructions

When a user requests help with Skillet formulas:

### Step 1: Understand the Requirement

Ask clarifying questions if needed:
- What data are you working with? (numbers, arrays, JSON objects)
- What's the desired output?
- Do you need CLI usage or HTTP API?
- Are there any null values to handle?
- Is performance critical (large datasets)?

### Step 2: Design the Formula

Consider:
1. **Input Format:** Variables, JSON structure, or hardcoded values?
2. **Operations Needed:** Filtering, mapping, aggregation, conditionals?
3. **Null Safety:** Will there be null values? Use `&.` or `.to_*()`
4. **Output Format:** Single value, array, or JSON object?

### Step 3: Build the Expression

**Structure Pattern:**
```bash
# 1. Variable assignments (if needed)
:var1 := expression1;
:var2 := expression2;

# 2. Main calculation
:result := main_expression;

# 3. Return final result (last expression is returned)
:result
```

**For Simple Calculations:**
```bash
:price * :quantity * (1 + :tax_rate)
```

**For Complex Logic:**
```bash
:subtotal := :price * :qty;
:tax := :subtotal * 0.08;
:shipping := IF(:subtotal > 100, 0, 10);
:total := :subtotal + :tax + :shipping;
:total
```

### Step 4: Provide Usage Examples

**ALWAYS provide BOTH CLI and HTTP examples:**

#### CLI Usage Template:
```bash
# Basic formula
cargo run --bin sk -- "formula_here"

# With variables (key=value)
cargo run --bin sk -- "formula_here" var1=value1 var2=value2

# With JSON input
cargo run --bin sk -- "formula_here" --json '{"key": "value"}'
```

#### HTTP API Usage Template:

**Format 1: Single String (for simple formulas)**
```bash
curl -X POST http://localhost:5074/eval \
  -H "Content-Type: application/json" \
  -d '{
    "expression": ":result := :var1 + :var2; :result",
    "arguments": {
      "var1": value1,
      "var2": value2
    }
  }'
```

**Format 2: Array of Strings (RECOMMENDED for complex formulas)**
```bash
curl -X POST http://localhost:5074/eval \
  -H "Content-Type: application/json" \
  -d '{
    "expression": [
      ":step1 := :var1 * :var2;",
      ":step2 := :step1 + 100;",
      ":result := :step2 * 1.16;",
      ":result"
    ],
    "arguments": {
      "var1": value1,
      "var2": value2
    },
    "include_variables": true
  }'
```

**Format 3: Array with Indentation (for very complex formulas)**
```bash
curl -X POST http://localhost:5074/eval \
  -H "Content-Type: application/json" \
  -d '{
    "expression": [
      ":filtered := :data.filter(:x.status == \"active\");",
      ":result := :filtered.map({",
      "  id: :item.id,",
      "  total: :item.price * :item.qty,",
      "  discounted: :item.price * :item.qty * 0.9",
      "}, '\''item'\'');",
      ":result"
    ],
    "arguments": {...}
  }'
```

**GET Request (simple formulas only):**
```bash
curl "http://localhost:5074/eval?expr=:a+:b&a=10&b=20"
```

### Step 5: Add Testing Commands

Provide a test case to validate the formula:
```bash
# Test case
cargo run --bin sk -- "formula" test_var1=test_value1
# Expected output: [describe expected result]
```

### Step 6: Optimize and Explain

- **Explain each part** of the formula
- **Suggest optimizations** if applicable
- **Mention edge cases** and how they're handled
- **Provide alternative approaches** if relevant

## Example Workflows

### Example 1: E-commerce Pricing Calculator

**User Request:** "Calculate total price with tax and shipping. Free shipping over $100."

**Response:**

```bash
# Formula
:subtotal := :price * :quantity;
:tax := :subtotal * :tax_rate;
:shipping := IF(:subtotal >= 100, 0, :shipping_fee);
:total := :subtotal + :tax + :shipping;
:total
```

**CLI Usage:**
```bash
cargo run --bin sk -- ":subtotal := :price * :quantity; :tax := :subtotal * :tax_rate; :shipping := IF(:subtotal >= 100, 0, :shipping_fee); :total := :subtotal + :tax + :shipping; :total" \
  price=19.99 \
  quantity=3 \
  tax_rate=0.08 \
  shipping_fee=10
# Output: Number(74.7676)
```

**HTTP API Usage (Array Format - Readable):**
```bash
curl -X POST http://localhost:5074/eval \
  -H "Content-Type: application/json" \
  -d '{
    "expression": [
      ":subtotal := :price * :quantity;",
      ":tax := :subtotal * :tax_rate;",
      ":shipping := IF(:subtotal >= 100, 0, :shipping_fee);",
      ":total := :subtotal + :tax + :shipping;",
      ":total"
    ],
    "arguments": {
      "price": 19.99,
      "quantity": 3,
      "tax_rate": 0.08,
      "shipping_fee": 10
    },
    "include_variables": true
  }'
```

**HTTP API Usage (Single String - Compact):**
```bash
curl -X POST http://localhost:5074/eval \
  -H "Content-Type: application/json" \
  -d '{
    "expression": ":subtotal := :price * :quantity; :tax := :subtotal * :tax_rate; :shipping := IF(:subtotal >= 100, 0, :shipping_fee); :total := :subtotal + :tax + :shipping; :total",
    "arguments": {
      "price": 19.99,
      "quantity": 3,
      "tax_rate": 0.08,
      "shipping_fee": 10
    },
    "include_variables": true
  }'
```

**Explanation:**
- `subtotal`: Base price calculation
- `tax`: Applied as percentage of subtotal
- `shipping`: Conditional - free if subtotal >= $100
- `total`: Sum of all components
- Result includes all intermediate variables for debugging

---

### Example 2: Data Filtering & Transformation

**User Request:** "Filter products over $50, apply 10% discount, sum the total."

**Response:**

```bash
# Formula with method chaining
:products.filter(:x.price > 50).map(:x.price * 0.9).sum()
```

**Alternative with SUMIF:**
```bash
# More concise for this specific case
SUMIF(:products.map(:x.price), :x > 50) * 0.9
```

**CLI Usage (with JSON):**
```bash
cargo run --bin sk -- ":products.filter(:x.price > 50).map(:x.price * 0.9).sum()" \
  --json '{
    "products": [
      {"name": "Widget", "price": 25},
      {"name": "Gadget", "price": 75},
      {"name": "Doohickey", "price": 120}
    ]
  }'
# Output: Number(175.5) [=(75 + 120) * 0.9]
```

**HTTP API Usage:**
```bash
curl -X POST http://localhost:5074/eval \
  -H "Content-Type: application/json" \
  -d '{
    "expression": ":products.filter(:x.price > 50).map(:x.price * 0.9).sum()",
    "arguments": {
      "products": [
        {"name": "Widget", "price": 25},
        {"name": "Gadget", "price": 75},
        {"name": "Doohickey", "price": 120}
      ]
    }
  }'
```

**Explanation:**
- `.filter(:x.price > 50)`: Keep only products over $50
- `.map(:x.price * 0.9)`: Apply 10% discount to prices
- `.sum()`: Sum the discounted prices
- Method chaining makes the logic clear and readable

---

### Example 3: JSONPath Complex Query

**User Request:** "Sum all sales from 'electronics' category using JSONPath."

**Response:**

```bash
# Using JQ function with JSONPath filtering
SUM(JQ(:arguments, "$.sales[?(@.category == 'electronics')].amount"))
```

**CLI Usage:**
```bash
cargo run --bin sk -- "SUM(JQ(:arguments, \"$.sales[?(@.category == 'electronics')].amount\"))" \
  --json '{
    "sales": [
      {"category": "electronics", "amount": 100},
      {"category": "books", "amount": 50},
      {"category": "electronics", "amount": 200},
      {"category": "clothing", "amount": 75}
    ]
  }'
# Output: Number(300.0)
```

**HTTP API Usage:**
```bash
curl -X POST http://localhost:5074/eval \
  -H "Content-Type: application/json" \
  -d '{
    "expression": "SUM(JQ(:arguments, \"$.sales[?(@.category == '\''electronics'\'')].amount\"))",
    "arguments": {
      "sales": [
        {"category": "electronics", "amount": 100},
        {"category": "books", "amount": 50},
        {"category": "electronics", "amount": 200},
        {"category": "clothing", "amount": 75}
      ]
    }
  }'
```

**Explanation:**
- `JQ(:arguments, "...")`: Executes JSONPath query on input data
- `$.sales[?(@.category == 'electronics')]`: Filters sales by category
- `.amount`: Extracts amount field from each match
- `SUM(...)`: Aggregates the filtered amounts
- More powerful than `.filter()` for complex JSON queries

---

### Example 4: Null-Safe Customer Data Processing

**User Request:** "Filter customers with valid email addresses, handling nulls."

**Response:**

```bash
# Using conversion methods for null safety
:customers.filter(:x.email.to_s().length() > 0)
```

**Alternative with safe navigation:**
```bash
# Preserves nulls but doesn't filter them out
:customers.map(:x&.email&.length())
```

**CLI Usage:**
```bash
cargo run --bin sk -- ":customers.filter(:x.email.to_s().length() > 0)" \
  --json '{
    "customers": [
      {"name": "Alice", "email": "alice@example.com"},
      {"name": "Bob", "email": null},
      {"name": "Charlie", "email": ""},
      {"name": "Diana", "email": "diana@example.com"}
    ]
  }'
# Output: Array with Alice and Diana only
```

**HTTP API Usage:**
```bash
curl -X POST http://localhost:5074/eval \
  -H "Content-Type: application/json" \
  -d '{
    "expression": ":customers.filter(:x.email.to_s().length() > 0)",
    "arguments": {
      "customers": [
        {"name": "Alice", "email": "alice@example.com"},
        {"name": "Bob", "email": null},
        {"name": "Charlie", "email": ""},
        {"name": "Diana", "email": "diana@example.com"}
      ]
    }
  }'
```

**Explanation:**
- `.to_s()`: Converts null to empty string (safe default)
- `.length() > 0`: Filters out empty strings and nulls
- No errors thrown even if email is null
- Alternative: Use `&.` to preserve null values without conversion

---

### Example 5: Business Logic with IFS

**User Request:** "Calculate employee bonus: 10% if sales > $100k, 6% if > $50k, 4% otherwise."

**Response:**

```bash
# Using IFS for multiple conditions
:bonus_rate := IFS(
  :sales > 100000, 0.10,
  :sales > 50000, 0.06,
  true, 0.04
);
:bonus := :salary * :bonus_rate;
:bonus
```

**Alternative with nested IF:**
```bash
:bonus := :salary * IF(:sales > 100000, 0.10, IF(:sales > 50000, 0.06, 0.04))
```

**CLI Usage:**
```bash
cargo run --bin sk -- ":bonus_rate := IFS(:sales > 100000, 0.10, :sales > 50000, 0.06, true, 0.04); :bonus := :salary * :bonus_rate; :bonus" \
  sales=75000 \
  salary=60000
# Output: Number(3600.0) [=60000 * 0.06]
```

**HTTP API Usage:**
```bash
curl -X POST http://localhost:5074/eval \
  -H "Content-Type: application/json" \
  -d '{
    "expression": ":bonus_rate := IFS(:sales > 100000, 0.10, :sales > 50000, 0.06, true, 0.04); :bonus := :salary * :bonus_rate; :bonus",
    "arguments": {
      "sales": 75000,
      "salary": 60000
    },
    "include_variables": true
  }'
```

**Explanation:**
- `IFS`: Checks conditions in order, returns first match
- `true` as final condition: Catch-all/default case
- `include_variables: true`: Returns bonus_rate for debugging
- Cleaner than nested IF statements for multiple conditions

---

### Example 6: Financial Calculation (Loan Payment)

**User Request:** "Calculate monthly mortgage payment for $300k loan, 4.5% APR, 30 years."

**Response:**

```bash
# PMT function: rate per period, number of periods, present value
PMT(0.045/12, 30*12, 300000)
```

**With variables for clarity:**
```bash
:monthly_rate := :annual_rate / 12;
:num_payments := :years * 12;
:monthly_payment := PMT(:monthly_rate, :num_payments, :loan_amount);
:monthly_payment
```

**CLI Usage:**
```bash
cargo run --bin sk -- "PMT(0.045/12, 30*12, 300000)"
# Output: Number(-1520.06) [negative indicates outflow]

# With variables
cargo run --bin sk -- ":monthly_rate := :annual_rate / 12; :num_payments := :years * 12; PMT(:monthly_rate, :num_payments, :loan_amount)" \
  annual_rate=0.045 \
  years=30 \
  loan_amount=300000
```

**HTTP API Usage:**
```bash
curl -X POST http://localhost:5074/eval \
  -H "Content-Type: application/json" \
  -d '{
    "expression": ":monthly_rate := :annual_rate / 12; :num_payments := :years * 12; PMT(:monthly_rate, :num_payments, :loan_amount)",
    "arguments": {
      "annual_rate": 0.045,
      "years": 30,
      "loan_amount": 300000
    }
  }'
```

**Explanation:**
- `PMT(rate, nper, pv)`: Excel-compatible payment function
- Rate and periods must be in same units (monthly)
- Negative result = money out (payment)
- Positive result = money in (investment return)

---

### Example 7: Date Calculations

**User Request:** "Calculate days until project deadline and if we're within 7 days."

**Response:**

```bash
:deadline := DATE(2026, 12, 31);
:days_remaining := DATEDIFF(NOW(), :deadline, "days");
:urgent := :days_remaining <= 7;
:urgent
```

**CLI Usage:**
```bash
cargo run --bin sk -- ":deadline := DATE(2026, 12, 31); :days_remaining := DATEDIFF(NOW(), :deadline, \"days\"); :urgent := :days_remaining <= 7; :urgent"
# Output: Boolean (true or false depending on current date)
```

**HTTP API Usage:**
```bash
curl -X POST http://localhost:5074/eval \
  -H "Content-Type: application/json" \
  -d '{
    "expression": ":deadline := DATE(2026, 12, 31); :days_remaining := DATEDIFF(NOW(), :deadline, \"days\"); :urgent := :days_remaining <= 7; {\"days_remaining\": :days_remaining, \"urgent\": :urgent}",
    "arguments": {},
    "include_variables": true
  }'
```

**Explanation:**
- `DATE(year, month, day)`: Create timestamp
- `NOW()`: Current timestamp
- `DATEDIFF(d1, d2, "days")`: Days between dates
- Returns object with both values for client use

---

## Common Patterns Library

### Pattern 0: HTTP Array Expression Formatting

**Simple Multi-Step Calculation:**
```json
{
  "expression": [
    ":subtotal := :price * :qty;",
    ":tax := :subtotal * :tax_rate;",
    ":total := :subtotal + :tax;",
    ":total"
  ]
}
```

**Complex Object Creation with Indentation:**
```json
{
  "expression": [
    ":result := {",
    "  id: :user_id,",
    "  full_name: CONCAT(:first_name, ' ', :last_name),",
    "  age: :birth_year - YEAR(NOW()),",
    "  status: IF(:active, 'Active', 'Inactive')",
    "};",
    ":result"
  ]
}
```

**Nested Function Calls (IFS with Indentation):**
```json
{
  "expression": [
    ":discount := IFS(",
    "  :qty >= 100, 0.20,",
    "  :qty >= 50, 0.15,",
    "  :qty >= 10, 0.10,",
    "  true, 0",
    ");",
    ":final_price := :price * (1 - :discount);",
    ":final_price"
  ]
}
```

**Complex Map with Object Return:**
```json
{
  "expression": [
    ":items := :products.map({",
    "  name: :p.name,",
    "  original: :p.price,",
    "  discounted: :p.price * 0.9,",
    "  savings: :p.price * 0.1",
    "}, 'p');",
    ":items"
  ]
}
```

**Multi-Level Filtering and Transformation:**
```json
{
  "expression": [
    ":active := :users.filter(:x.status == 'active');",
    ":with_orders := :active.filter(:x.order_count > 0);",
    ":enriched := :with_orders.map({",
    "  id: :u.id,",
    "  name: :u.name,",
    "  lifetime_value: :u.order_total * 1.16,",
    "  tier: IFS(",
    "    :u.order_total > 10000, 'platinum',",
    "    :u.order_total > 5000, 'gold',",
    "    :u.order_total > 1000, 'silver',",
    "    true, 'bronze'",
    "  )",
    "}, 'u');",
    ":enriched"
  ]
}
```

**When to Break Lines:**
- ✅ Each variable assignment gets its own line
- ✅ Object properties each on separate lines (indented)
- ✅ IFS/IF conditions each on separate lines (indented)
- ✅ Complex map/filter lambdas broken into multiple lines
- ✅ Final return expression on its own line

### Pattern 1: Conditional Aggregation
```bash
# Sum only values above threshold
SUMIF(:data, :x > :threshold)

# Average of positive values
AVGIF(:data, :x > 0)

# Count items matching criteria
COUNTIF(:data, :x.status == "active")
```

### Pattern 2: Data Transformation Pipeline
```bash
# Filter → Transform → Aggregate
:data
  .filter(:x.status == "active")
  .map(:x.value * :multiplier)
  .sum()
```

### Pattern 3: Null-Safe Property Access
```bash
# Using conversion methods
:items.map(:x.name.to_s().upper())

# Using safe navigation
:items.map(:x&.name&.upper())

# Filtering nulls
:items.filter(:x.value.to_s().length() > 0)
```

### Pattern 4: Complex Conditionals
```bash
# Nested IF
IF(:tier == "premium",
  :price * 0.8,
  IF(:tier == "standard", :price * 0.9, :price)
)

# IFS (cleaner)
IFS(
  :tier == "premium", :price * 0.8,
  :tier == "standard", :price * 0.9,
  true, :price
)

# Ternary (simple cases)
:active ? :full_price : :discounted_price
```

### Pattern 5: Array Manipulation
```bash
# Flatten nested arrays
FLATTEN(:nested_data)

# Remove duplicates
UNIQUE(:data)

# Remove nulls
COMPACT(:data)

# Combined
:data.flatten().unique().compact()
```

### Pattern 6: String Formatting
```bash
# Concatenation
CONCAT(:first_name, " ", :last_name)

# Method chaining
:input.trim().upper().reverse()

# Substring extraction
LEFT(:product_code, 3)  # First 3 chars
RIGHT(:zip_code, 4)     # Last 4 chars
MID(:text, 5, 10)       # Middle section
```

### Pattern 7: Statistical Analysis
```bash
# Basic stats
:scores.avg()           # Average
:scores.min()           # Minimum
:scores.max()           # Maximum
MEDIAN(:scores)         # Median
STDEV_P(:scores)        # Std deviation

# Percentile analysis
PERCENTILE_INC(:scores, 0.95)  # 95th percentile
QUARTILE_INC(:scores, 3)       # Q3
```

### Pattern 8: Object Creation & Manipulation
```bash
# Create object literal
:result := {
  total: :subtotal + :tax,
  discounted: :subtotal * 0.9,
  tax_amount: :tax
};
:result

# Navigate nested objects
DIG(:data, ['user', 'profile', 'settings', 'theme'])

# Extract keys/values
:config.keys()          # Array of keys
:config.values()        # Array of values
```

## Troubleshooting Guide

### Common Errors and Solutions

**Error: "Missing variable: :x"**
- **Cause:** Variable not provided or typo in name
- **Fix:** Check variable names match exactly (case-sensitive)
```bash
# ✅ Correct
cargo run --bin sk -- ":price * :qty" price=10 qty=5

# ❌ Wrong (typo in qty)
cargo run --bin sk -- ":price * :qty" price=10 quantity=5
```

**Error: "No methods available for Null type"**
- **Cause:** Calling method on null without null safety
- **Fix:** Use `.to_*()` methods or `&.` operator
```bash
# ❌ Error if :name is null
:name.length()

# ✅ Safe - returns 0 if null
:name.to_s().length()

# ✅ Safe - returns null if null
:name&.length()
```

**Error: "Type mismatch"**
- **Cause:** Operation on incompatible types
- **Fix:** Use type conversion or check types
```bash
# ❌ Error if :value is string
:value + 10

# ✅ Convert first
:value.to_i() + 10

# ✅ Check type
IF(ISNUMBER(:value), :value + 10, 0)
```

**Error: "Lambda parameter :x not found"**
- **Cause:** Using wrong parameter name in lambda
- **Fix:** Use correct parameter or specify custom name
```bash
# ❌ Wrong parameter name
[1,2,3].filter(:item > 2)  # Error: :item not defined

# ✅ Use default :x
[1,2,3].filter(:x > 2)

# ✅ Or specify custom name
[1,2,3].filter(:item > 2, 'item')
```

**Error: "Invalid JSON"**
- **Cause:** Malformed JSON in --json flag or HTTP body
- **Fix:** Validate JSON syntax (quotes, commas, brackets)
```bash
# ❌ Invalid JSON (missing quotes)
--json '{name: "John"}'

# ✅ Valid JSON
--json '{"name": "John"}'
```

## Output Format

When providing Skillet formulas, ALWAYS include:

1. **Formula** - The Skillet expression (clearly formatted)
2. **CLI Example** - Command to test via cargo run
3. **HTTP Example** - curl command for API usage (use array format for 3+ statements)
4. **Explanation** - What each part does
5. **Test Case** - Expected output with sample data
6. **Edge Cases** - Null handling, empty arrays, etc.

**HTTP API Format Guidelines:**
- ✅ **Use array format** for formulas with 3+ statements
- ✅ **Use single string** only for very simple (1-2 statement) formulas
- ✅ **Indent complex expressions** (object literals, IFS, nested maps)
- ✅ **One logical statement per array element**
- ✅ **End each statement with semicolon** `;` (except final expression)

## Best Practices Checklist

Before delivering a formula, verify:

- ✅ Variable names use `:` prefix
- ✅ Null safety considered (use `&.` or `.to_*()`)
- ✅ Appropriate function/method used (built-in vs custom logic)
- ✅ Performance optimized (avoid redundant calculations)
- ✅ Both CLI and HTTP examples provided
- ✅ **HTTP examples use array format for complex formulas** (3+ statements)
- ✅ Explanation is clear and complete
- ✅ Test case with expected output included
- ✅ Edge cases mentioned
- ✅ Proper indentation in array format for readability

## Advanced Techniques

### Technique 1: Variable Reuse for Performance
```bash
# ✅ Calculate once, reuse
:tax_rate := 1.16;
:subtotal := :price * :qty;
:total := :subtotal * :tax_rate;
:total

# ❌ Redundant calculations
(:price * :qty) * 1.16
```

### Technique 2: JSONPath vs Filter
```bash
# Use JQ for complex nested queries
SUM(JQ(:arguments, "$.departments[*].employees[*].salary"))

# Use .filter() for simple array filtering
:employees.filter(:x.department == "Engineering")
```

### Technique 3: Excel-Style vs Lambda Criteria
```bash
# Excel-style (simple)
SUMIF([10, 20, 30, 40], ">25")

# Lambda (complex logic)
SUMIF(:data, :x.amount > :threshold AND :x.active)
```

### Technique 4: Object Literals for Complex Returns
```bash
# Return multiple calculated values as object
{
  subtotal: :price * :qty,
  tax: :price * :qty * :tax_rate,
  total: :price * :qty * (1 + :tax_rate),
  discounted: :price * :qty * 0.9
}
```

### Technique 5: Reduce for Custom Aggregations
```bash
# Custom aggregation: sum of squares
[1,2,3,4,5].reduce(:acc + (:x * :x), 0)
# Result: 55 (1² + 2² + 3² + 4² + 5²)

# Find maximum with reduce
:data.reduce(IF(:x > :acc, :x, :acc), 0)
```

## Integration Patterns

### Pattern: Ruby on Rails Integration
```ruby
# In Rails controller/model
require 'net/http'
require 'json'

def calculate_price(price, qty, tax_rate)
  uri = URI('http://localhost:5074/eval')
  response = Net::HTTP.post_form(uri, {
    expression: ':price * :qty * (1 + :tax_rate)',
    arguments: { price: price, qty: qty, tax_rate: tax_rate }.to_json
  })

  result = JSON.parse(response.body)
  result['result']
end
```

### Pattern: JavaScript/Node.js Integration
```javascript
// Using fetch API
async function evaluateSkillet(expression, variables) {
  const response = await fetch('http://localhost:5074/eval', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      expression: expression,
      arguments: variables
    })
  });

  const result = await response.json();
  return result.result;
}

// Usage
const total = await evaluateSkillet(
  ':price * :qty * (1 + :tax_rate)',
  { price: 19.99, qty: 3, tax_rate: 0.08 }
);
```

### Pattern: Python Integration
```python
import requests
import json

def evaluate_skillet(expression, variables):
    response = requests.post('http://localhost:5074/eval',
        headers={'Content-Type': 'application/json'},
        json={
            'expression': expression,
            'arguments': variables
        }
    )
    return response.json()['result']

# Usage
total = evaluate_skillet(
    ':price * :qty * (1 + :tax_rate)',
    {'price': 19.99, 'qty': 3, 'tax_rate': 0.08}
)
```

## Final Checklist

Before providing a solution:

1. ✅ **Understand** - Clarified requirements and data structure
2. ✅ **Design** - Chose appropriate functions/methods
3. ✅ **Implement** - Wrote clear, efficient formula
4. ✅ **Test** - Provided working CLI example
5. ✅ **Integrate** - Provided HTTP API example
6. ✅ **Explain** - Documented what each part does
7. ✅ **Optimize** - Checked for performance issues
8. ✅ **Handle Errors** - Considered null/edge cases
9. ✅ **Format** - Clean, readable presentation

You are now ready to help users create powerful, efficient Skillet formulas for any use case!
