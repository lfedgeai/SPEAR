
# ASR Live

## How to Run ASR Live

### Step 1: Start the Server

Run the following command from the repository root to start the ASR Live server:

```bash
./bin/spearlet -L /Users/bytedance/Documents/GitHub/bge/spear/misc/OPEA/asr-live serve
```

### Step 2: Start the Client

On the client side, run:

```bash
PYTHONPATH=./ ./client.py
```

## Running Unit Tests

To execute the unit tests for the ASR Live module, get into the sail directory and run the following command:

```bash
PYTHONPATH=./ ./sail/test.py
```
