let ethers = require('ethers')

let url = 'http://127.0.0.1:8545'
let provider = new ethers.providers.StaticJsonRpcProvider(url)

let output = new Map()
output.set("context", new Map())
output.set("states", new Map())

const OP_SSTORE = 'SSTORE'
const OP_SLOAD = 'SLOAD'
const OP_CALL = 'CALL'
const OP_STATICCALL = 'STATICCALL'
const OP_CALLCODE = 'CALLCODE'
const OP_DELEGATECALL = 'DELEGATECALL'

function getContractState(addr) {
    let states = output.get("states")
    if (states.has(addr))
        return states.get(addr)
    let contract = {
        address: addr,
        partial_storage_before: new Map(),
        partial_storage_after: new Map(),
        code: ""
    }
    states.set(addr, contract)
    return contract
}

async function setContractCode(addr) {
    let contract = getContractState(addr)
    if (contract.code === '') {
        contract.code = await provider.getCode(addr)
    }
}

async function main() {
    let txHash = '0x5ec8485eb215f91646c34672e0323a4da4b88866c6f89f44f252b24de9b3fcf0'
    let tx = await provider.getTransaction(txHash)
    let traceResult = await provider.send("debug_traceTransaction", [txHash])

    let initialStorageOwner
    if (tx.to === null) {
        initialStorageOwner = tx.creates
    } else {
        initialStorageOwner = tx.to
    }
    // fetch contract code
    await setContractCode(initialStorageOwner)

    // transaction context
    let transactionContext = output.get("context")
    // transaction info
    transactionContext.set('from', tx.from)
    transactionContext.set('to', tx.to)
    transactionContext.set('input', tx.data)
    transactionContext.set('value', tx.value)
    // block info
    transactionContext.set("block_number", tx.blockNumber)
    let block = await provider.getBlock(tx.blockNumber)
    transactionContext.set("timestamp", block.timestamp)
    transactionContext.set('block_hash', block.hash)
    transactionContext.set('block_difficulty', block.difficulty)
    // exec result
    transactionContext.set('status', traceResult.failed ? 0 : 1)
    transactionContext.set('return', traceResult.returnValue)


    let storageOwners = []
    storageOwners.push(initialStorageOwner.toLowerCase())

    let depth = 1
    for (let i = 0; i < traceResult.structLogs.length - 1; i++) {
        let log = traceResult.structLogs[i]
        if (depth < log.depth) {
            storageOwners.pop()
            depth = log.depth
        }

        switch (log.op) {
            case OP_SSTORE: {
                let key = log.stack[log.stack.length - 1]
                let val = log.stack[log.stack.length - 2]
                let contract = getContractState(storageOwners[storageOwners.length - 1])
                contract.partial_storage_after.set(key, val)
                break
            }
            case OP_SLOAD: {
                let key = log.stack[log.stack.length - 1]
                let nextLog = traceResult.structLogs[i + 1]
                if (nextLog != undefined) {
                    let val = nextLog.stack[nextLog.stack.length - 1]
                    let contract = getContractState(storageOwners[storageOwners.length - 1])
                    if (!contract.partial_storage_before.has(key)) {
                        contract.partial_storage_before.set(key, val)
                    }
                    contract.partial_storage_after.set(key, val)
                } else {
                    console.warn("no next log")
                }
                break
            }
            case OP_CALL: {
                depth++
                let address = log.stack[log.stack.length - 2]
                await setContractCode(address)
                storageOwners.push(address)
                break
            }
            case OP_STATICCALL: {
                depth++
                let address = log.stack[log.stack.length - 2]
                await setContractCode(address)
                storageOwners.push(address)
                break
            }
            case OP_DELEGATECALL: {
                depth++
                let address = log.stack[log.stack.length - 2]
                await setContractCode(address)
                storageOwners.push(storageOwners[storageOwners.length - 1])
                break
            }
            case OP_CALLCODE: {
                depth++
                let address = log.stack[log.stack.length - 2]
                await setContractCode(address)
                storageOwners.push(storageOwners[storageOwners.length - 1])
                break
            }
        }
    }
}

main().then(() => {
    console.log(output)
})
