# MAP Protocol Ori

Implementation of MAP Protocol PoS consensus in Rust.

# Quick Start

Following steps will help you to get started building with MAP Ori poc0 on linux(Ubuntu).

make sure rust installed and install dependency 

```shell
sudo apt install llvm clang
```
Get the project:
```shell
git clone https://github.com/mapprotocol/ori.git
cd ori
```
compile and run 
```shell
cargo build
cargo run
```

### Run Map
```shell script
$  target\debug\map --key "0xf9cb7ea173840aeba4fc8146743464cdae3e5527414872155fe331bd2a3454a2"
```

This command explain:
 * `--key` default private key.
  
**Output Log**
```shell
[2020-03-28T04:08:55Z INFO ] using datadir .
[2020-03-28T04:08:55Z INFO ] setup genesis hash=0x02245e11
[2020-03-28T04:08:55Z INFO ] using url 127.0.0.1:9545
[2020-03-28T04:08:55Z INFO ] seal block, height=1, parent=0x02245e11, tx=0
[2020-03-28T04:08:55Z INFO ] sign block with genesis privkey, height=1, hash=0xfc8ffdba
[2020-03-28T04:08:55Z INFO ] insert block, height=1, hash=0x8781fa14, previous=0x02245e11
[2020-03-28T04:08:57Z INFO ] seal block, height=2, parent=0x8781fa14, tx=0
[2020-03-28T04:08:57Z INFO ] sign block with genesis privkey, height=2, hash=0x31cc1e45
[2020-03-28T04:08:57Z INFO ] insert block, height=2, hash=0x4a55eb26, previous=0x8781fa14
```

### RPC API

#### map_sendTransaction

```
$ curl -d '{"id": 2, "jsonrpc": "2.0", "method":"map_sendTransaction","params": ["0xd2480451ef35ff2fdd7c69cad058719b9dc4d631","0x0000000000000000000000000000000000000011",1000000000]}' -H 'content-type:application/json' 'http://localhost:9545'
```

This command explain:
 * `--params` send transaction params.
     - `first`:  - address of the from.
     - `second`: - to  address
     - `third`: - transfer value
 * `--localhost` connect local.
 * `--9545`     - default port
  
**Output Log**
```shell
{"jsonrpc":"2.0","result":"0x90ed7db8","id":2}
```

#### map_getBlockByNumber

```
$ curl -d '{"id": 2, "jsonrpc": "2.0", "method":"map_getBlockByNumber","params": [44]}' -H 'content-type:application/json' 'http://localhost:9545'

```

This command explain:
 * `--params` block number.
 * `--localhost` connect local.
 * `--9545`     - default port

**Output Log**
```shell
{"jsonrpc":"2.0","result":{
                            "header":{
                                    "height":105,"
                                     parent_hash":"0x65c6bd159129975a457acc4bd664e40ce41005d54ac5d05cbf7c72d0acba9e9d",
                                    "sign_root":"0x6307f8519e94bd870ae73bf525f508af548c935320eaa1528d61eade3fedfde2",
                                    "state_root":"0x4b45d631fcac62cb5639e7cbdf0ed97a290c2aa1cbd8fbe377681235cb778d33",
                                    "time":1585368746,
                                    "tx_root":"0xbbc2bf205e8735c3471e288c7e209e47fee56bb785de1c9244b23cba2edd325b"},
                            "proofs":[],
                            "signs":[{"msg":"0x0f049acd3e15af39be612b140c23c9a467585f871bd801ed006c76e05c8cf3c2",
                                    "signs":[[215,154,44,153,71,108,1,198,10,28,181,239,160,138,108,227,144,223,8,183,97,151,106,79,255,178,166,24,63,46,114,76],
                                            [224,123,143,121,122,136,119,88,121,187,24,148,186,112,146,116,183,106,12,28,91,164,154,138,112,17,106,233,134,254,99,5],
                                            [243,168,124,46,165,43,188,124,215,100,221,215,249,71,217,60,226,13,9,72,114,24,80,73,118,31,251,38,82,192,147,7]]
                                    }],
                            "txs":[]
                          },
                   "id":2}
```

#### map_getHeaderByNumber

```
$ curl -d '{"id": 2, "jsonrpc": "2.0", "method":"map_getBlockByNumber","params": [44]}' -H 'content-type:application/json' 'http://localhost:9545'

```

This command explain:
 * `--params` head number.
 * `--localhost` connect local.
 * `--9545`     - default port

**Output Log**
```shell
{"jsonrpc":"2.0","result":{
                            "height":14,
                            "parent_hash":"0x1470bfb3ac1e42c6be82be1cc366dd6161bffa10ec32fd0709eb0a8b27a9b2c7",
                            "sign_root":"0x7a4d940f41fd53c2a6f49d76d73bb3e95c81f71a4e883b9b2062cdc339e952b1",
                            "state_root":"0x4b45d631fcac62cb5639e7cbdf0ed97a290c2aa1cbd8fbe377681235cb778d33",
                            "time":1585368561,
                            "tx_root":"0xc567b28462d6b766e2c76761d43a1d734f3c6196d48897770b1647caeff190f3"},
                    "id":2}
```
