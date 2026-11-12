name: Deploy and Run Base MEV Bot

on:
  workflow_dispatch:

jobs:
  run-bot:
    runs-on: ubuntu-latest
    timeout-minutes: 360

    steps:
      - name: Checkout Repository
        uses: actions/checkout@v4

      - name: Setup Rust Toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy

      - name: Install Foundry (Anvil & Cast)
        uses: foundry-rs/foundry-toolchain@v1
        with:
          version: nightly

      - name: Start Local Anvil Fork in Background
        env:
          ALCHEMY_HTTP_URL: ${{ secrets.ALCHEMY_HTTP_URL }}
        run: |
          echo "[*] Starting local Anvil fork of Base Mainnet..."
          nohup anvil --fork-url "$ALCHEMY_HTTP_URL" --port 8545 > anvil.log 2>&1 &
          echo "[*] Waiting for Anvil RPC to initialize..."
          
          for i in {1..20}; do
            if curl -s -X POST -H "Content-Type: application/json" --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' http://localhost:8545 > /dev/null; then
              echo "[*] Anvil is up and running!"
              break
            fi
            echo "[*] Waiting for Anvil RPC ($i/20)..."
            sleep 2
          done

          if ! curl -s -X POST -H "Content-Type: application/json" --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' http://localhost:8545 > /dev/null; then
            echo "[!] Anvil failed to start. Logs:"
            cat anvil.log
            exit 1
          fi

      - name: Verify Contract State & Ownership
        env:
          CONTRACT_ADDR: ${{ secrets.CONTRACT_ADDR }}
          PRIVATE_KEY: ${{ secrets.PRIVATE_KEY }}
        run: |
          echo "======================================================="
          echo "🔍 [VERIFY] Checking Deployed Contract on Base Fork..."
          echo "======================================================="
          
          # تنظيف المتغيرات من أي أحرف خفية أو فواصل سطرية أو مسافات زائدة
          CLEAN_CONTRACT_ADDR=$(echo "$CONTRACT_ADDR" | tr -d '[:space:]')
          CLEAN_PRIVATE_KEY=$(echo "$PRIVATE_KEY" | tr -d '[:space:]')

          ACCOUNT_ADDR=$(cast wallet address --private-key "$CLEAN_PRIVATE_KEY")
          echo "[*] Bot Signer Address: $ACCOUNT_ADDR"

          # التحقق من المالك وعنوان الراوتر باستخدام المتغيرات المنظفة
          OWNER=$(cast call "$CLEAN_CONTRACT_ADDR" "owner()(address)" --rpc-url "http://127.0.0.1:8545")
          ROUTER=$(cast call "$CLEAN_CONTRACT_ADDR" "BALANCER_V3_BATCH_ROUTER()(address)" --rpc-url "http://127.0.0.1:8545")
          
          echo "[*] Contract Owner: $OWNER"
          echo "[*] Balancer V3 BatchRouter: $ROUTER"

          # تسجيل عنوان البوت كـ botAddress في العقد تلقائياً على الفورك المحلي للمحاكاة
          cast send "$CLEAN_CONTRACT_ADDR" "updateBotAddress(address)" "$ACCOUNT_ADDR" \
            --private-key "$CLEAN_PRIVATE_KEY" \
            --rpc-url "http://127.0.0.1:8545" > /dev/null 2>&1 || true

          echo "======================================================="

      - name: Cache Cargo Dependencies
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/bin/
            ~/.cargo/registry/index/
            ~/.cargo/registry/cache/
            ~/.cargo/git/db/
            target/
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Build Binary (Release)
        run: cargo build --release

      - name: Execute MEV Engine
        env:
          CONTRACT_ADDR: ${{ secrets.CONTRACT_ADDR }}
          PRIVATE_KEY: ${{ secrets.PRIVATE_KEY }}
          ALCHEMY_HTTP_URL: ${{ secrets.ALCHEMY_HTTP_URL }}
          ALCHEMY_WSS_URL: ${{ secrets.ALCHEMY_WSS_URL }}
          PRIVATE_RPC_URL: "http://127.0.0.1:8545"
        run: |
          echo "[*] Starting Balancer V3 BatchRouter MEV Engine..."
          cargo run --release
