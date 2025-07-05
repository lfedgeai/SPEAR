
# ASR Live

## How to Run ASR Live

### Step 1: Start the Server

Run the following command from the repository root to start the ASR Live server:

```bash
REPO_ROOT=<repo_root>
./bin/spearlet -L $REPO_ROOT/misc/OPEA/asr-live serve
```

### Step 2: Start the Client

On the client side, run:

```bash
REPO_ROOT=<repo_root>
cd $REPO_ROOT/misc/OPEA/asr-live
PYTHONPATH=./ ./ws_client.py
```

## Running Unit Tests

To execute the unit tests for the ASR Live module, get into the sail directory and run the following command:

```bash
PYTHONPATH=./ ./sail/test.py
```
