This was added to fix https://github.com/Lay3rLabs/wasmatic/issues/141

Component was copy/pasted from https://github.com/Lay3rLabs/avs-toolkit/tree/6cf715d4b33a49b507c2c67ac7cf228cdf101e69/wasi/ai-bouncer

Instructions below assume `avs-toolkit-cli` is installed globally (`cargo install --path ./tools/cli` in avs-toolkit repo)

To reproduce

### In real wasmatic test

1. Get wasmatic running and core contracts deployed: https://github.com/Lay3rLabs/my-layer/blob/main/localnode/DEMO.md#deploy-contracts

2. Deploy this component (from in `wasmatic` repo root):

Use the real `GROQ_API_KEY`

```
avs-toolkit-cli --target=local wasmatic deploy --name ai-bouncer \
--envs "PROVIDER=groq" \
--envs "GROQ_API_KEY=REDACTED" \
--wasm-source ./components/ai_bouncer.wasm  \
--testable \
--task $LOCAL_TASK_QUEUE_ADDRESS
```

3. Try to test it

(example message, address is random)

```
avs-toolkit-cli --target=local wasmatic test --name ai-bouncer \
--input "{ \
	\"sessionId\": \"1\", \
	\"address\": \"layer177eww0wg6zrvyxn9eaqzkjqmeqh36lxe3025gg\", \
	\"messageId\": 0, \
	\"message\": \"uhhh\" \
}" 
```

### In Direct Runner

Use the real `GROQ_API_KEY`

From `wasmatic` repo root:

(example message, address is random)

```
avs-toolkit-cli wasmatic run \
 --wasm-source ./components/ai_bouncer.wasm \
 --envs "PROVIDER=groq" \
 --envs "GROQ_API_KEY=REDACTED" \
 --input "{ \
	\"sessionId\": \"1\", \
	\"address\": \"layer177eww0wg6zrvyxn9eaqzkjqmeqh36lxe3025gg\", \
	\"messageId\": 0, \
	\"message\": \"uhhh\" \
 }"
 ```