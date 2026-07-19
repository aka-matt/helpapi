# helpapi-mock-wasm

WASM bindings for the HelpAPI mock engine. Use this package to run a mock API server directly in browser or Node.js environments.

## Installation

```bash
npm install helpapi-mock-wasm
```

## Usage

```javascript
import init, { WasmMockEngine, validate_config } from 'helpapi-mock-wasm';

// Initialize the WASM module
await init();

// Validate a configuration
const validationResult = validate_config(configJson);
if (!JSON.parse(validationResult).ok) {
  console.error('Invalid config');
}

// Create an engine
const engine = new WasmMockEngine(configJson);

// Make a decision
const decision = engine.decide(requestJson);
console.log(JSON.parse(decision));

// Transform a response
const transformed = engine.transform_response(contextJson, responseJson);
console.log(JSON.parse(transformed));
```

## API

### `validate_config(configJson: string): string`

Validates a JSON configuration string. Returns `{"ok": true}` on success or `{"ok": false, "error": "..."}` on failure.

### `WasmMockEngine`

#### `new WasmMockEngine(configJson: string)`

Creates a new engine from a JSON configuration string.

#### `engine.decide(requestJson: string): string`

Evaluates a request against the compiled rules. Returns a JSON string representing a `Decision` object.

#### `engine.transform_response(contextJson: string, responseJson: string): string`

Transforms an upstream response using the response transforms from the matched route.

## Configuration Format

```json
{
  "version": 1,
  "server": { "host": "127.0.0.1", "port": 8080 },
  "defaults": { "upstream_timeout_ms": 10000, "max_body_bytes": 1048576 },
  "routes": [
    {
      "id": "get-user",
      "priority": 100,
      "match": { "method": "GET", "path": "/users/:id" },
      "action": { "type": "mock", "response": { "status": 200, "json": { "id": 1 } } }
    }
  ]
}
```

## License

MIT
